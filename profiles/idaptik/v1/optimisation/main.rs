// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
//! Offline guard placement; the profile owns all problem vocabulary.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use ums_ai_edit::{engine, verbs};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Zone {
    id: String,
    min: usize,
    max: usize,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Guard {
    id: String,
    rank: String,
    /// Missing zone means forbidden, not zero cost.
    costs: BTreeMap<String, u32>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Problem {
    format: String,
    id: String,
    base_script: Value,
    zones: Vec<Zone>,
    guards: Vec<Guard>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Solution {
    assignment: BTreeMap<String, String>,
    cost: u64,
}

impl Problem {
    fn prepare(&mut self) -> Result<(), String> {
        if self.format != "idaptik-guard-placement/v1" || self.id.is_empty() {
            return Err("unknown format or empty instance id".into());
        }
        if self.guards.len() > 64 || self.zones.is_empty() || self.zones.len() > 16 {
            return Err("bounds: at most 64 guards and 1..=16 zones".into());
        }
        self.guards.sort_by(|a, b| a.id.cmp(&b.id));
        self.zones.sort_by(|a, b| a.id.cmp(&b.id));
        let unique = |ids: Vec<&str>| {
            ids.iter().all(|id| !id.is_empty())
                && ids.iter().copied().collect::<BTreeSet<_>>().len() == ids.len()
        };
        if !unique(self.guards.iter().map(|g| g.id.as_str()).collect())
            || !unique(self.zones.iter().map(|z| z.id.as_str()).collect())
        {
            return Err("empty or duplicate ids".into());
        }
        if self.zones.iter().any(|z| z.min > z.max || z.max > 64) {
            return Err("zone limits must satisfy 0 <= min <= max <= 64".into());
        }
        if self.base_script.get("profile").and_then(Value::as_str) != Some("idaptik")
            || self
                .base_script
                .get("edits")
                .and_then(Value::as_array)
                .is_none()
        {
            return Err("base_script needs profile idaptik and edits array".into());
        }
        let (base, report) = engine::apply_profile_edit_script(
            "idaptik",
            &verbs::initial_state(),
            &self.base_script,
        );
        if !report.ok {
            return Err(format!("invalid base script: {:?}", report.reason));
        }
        let base = engine::to_json(&base);
        let base_zones = base["zones"].as_array().ok_or("missing base zones")?;
        let ids: BTreeSet<_> = base_zones.iter().filter_map(|z| z["id"].as_str()).collect();
        if ids.len() != base_zones.len()
            || ids != self.zones.iter().map(|z| z.id.as_str()).collect()
            || !base["guards"]
                .as_array()
                .ok_or("missing base guards")?
                .is_empty()
        {
            return Err("base must have exactly the declared unique zones and no guards".into());
        }
        for g in &self.guards {
            if g.costs.keys().any(|z| !ids.contains(z.as_str())) {
                return Err(format!("{} refers to an unknown zone", g.id));
            }
            // Check ranks using the actual registry, even for ineligible guards.
            let probe = json!({"edits":[{"verb":"add_guard", "id":g.id,
                "rank":g.rank, "zone":self.zones[0].id}]});
            if !engine::apply_profile_edit_script("idaptik", &engine::from_json(&base), &probe)
                .1
                .ok
            {
                return Err(format!("{} has an invalid guard rank", g.id));
            }
        }
        Ok(())
    }

    fn script(&self, solution: &Solution) -> Value {
        let mut script = self.base_script.clone();
        let edits = script["edits"]
            .as_array_mut()
            .expect("prepared base script");
        for g in &self.guards {
            edits.push(json!({"verb":"add_guard", "id":g.id, "rank":g.rank,
                "zone":solution.assignment[&g.id]}));
        }
        script
    }
}

/// Independent complete-candidate validator: no solver state or incremental
/// counters are trusted. Recompute cost and quotas, then replay actual verbs.
fn validate(p: &Problem, s: &Solution) -> Result<Value, String> {
    if s.assignment.len() != p.guards.len() {
        return Err("assignment cardinality differs".into());
    }
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut cost = 0u64;
    for g in &p.guards {
        let zone = s.assignment.get(&g.id).ok_or("missing guard")?;
        cost += u64::from(*g.costs.get(zone).ok_or("forbidden guard/zone pair")?);
        *counts.entry(zone).or_default() += 1;
    }
    for z in &p.zones {
        let n = counts.get(z.id.as_str()).copied().unwrap_or(0);
        if n < z.min || n > z.max {
            return Err(format!("quota violated: {}", z.id));
        }
    }
    if cost != s.cost {
        return Err("reported objective differs from recomputed cost".into());
    }
    let script = p.script(s);
    let (_, report) =
        engine::apply_profile_edit_script("idaptik", &verbs::initial_state(), &script);
    if !report.ok {
        return Err(format!("edit replay rejected: {:?}", report.reason));
    }
    Ok(script)
}

fn candidate(p: &Problem, a: &[usize]) -> Solution {
    Solution {
        cost: a
            .iter()
            .enumerate()
            .map(|(i, &j)| u64::from(p.guards[i].costs[&p.zones[j].id]))
            .sum(),
        assignment: a
            .iter()
            .enumerate()
            .map(|(i, &j)| (p.guards[i].id.clone(), p.zones[j].id.clone()))
            .collect(),
    }
}

/// Enumerate every eligible vector (<= 2 million leaves), pruning only upper
/// quotas. Comparison includes the canonical assignment for deterministic ties.
fn exact(p: &Problem) -> Result<Option<Solution>, String> {
    let leaves = p
        .guards
        .iter()
        .try_fold(1usize, |n, g| n.checked_mul(g.costs.len()));
    if p.guards.len() > 12 || leaves.is_none_or(|n| n > 2_000_000) {
        return Err("exact limit: 12 guards and 2,000,000 eligible vectors".into());
    }
    fn visit(p: &Problem, a: &mut Vec<usize>, counts: &mut [usize], best: &mut Option<Solution>) {
        if a.len() == p.guards.len() {
            if p.zones.iter().enumerate().any(|(j, z)| counts[j] < z.min) {
                return;
            }
            let s = candidate(p, a);
            if best
                .as_ref()
                .is_none_or(|b| (s.cost, &s.assignment) < (b.cost, &b.assignment))
            {
                *best = Some(s);
            }
            return;
        }
        for (j, z) in p.zones.iter().enumerate() {
            if counts[j] < z.max && p.guards[a.len()].costs.contains_key(&z.id) {
                counts[j] += 1;
                a.push(j);
                visit(p, a, counts, best);
                a.pop();
                counts[j] -= 1;
            }
        }
    }
    let mut best = None;
    visit(p, &mut Vec::new(), &mut vec![0; p.zones.len()], &mut best);
    Ok(best)
}

/// Scarcity-first greedy placement, preferring unfilled lower quotas; then
/// best-improvement one-guard moves and two-guard swaps. No randomness.
/// Failure is "no_candidate", never an infeasibility certificate.
fn heuristic(p: &Problem) -> Option<Solution> {
    let mut order: Vec<_> = (0..p.guards.len()).collect();
    order.sort_by_key(|&i| (p.guards[i].costs.len(), &p.guards[i].id));
    let mut a = vec![0; p.guards.len()];
    let mut counts = vec![0; p.zones.len()];
    for i in order {
        let j = (0..p.zones.len())
            .filter(|&j| {
                counts[j] < p.zones[j].max && p.guards[i].costs.contains_key(&p.zones[j].id)
            })
            .min_by_key(|&j| {
                (
                    counts[j] >= p.zones[j].min,
                    p.guards[i].costs[&p.zones[j].id],
                    &p.zones[j].id,
                )
            })?;
        a[i] = j;
        counts[j] += 1;
    }
    if p.zones.iter().enumerate().any(|(j, z)| counts[j] < z.min) {
        return None;
    }
    let mut current = candidate(p, &a);
    // Bounded authoring latency. Each accepted step strictly improves cost or
    // canonical assignment; reaching this cap does not claim local optimality.
    for _ in 0..128 {
        let mut best = current.clone();
        let mut next = a.clone();
        let mut consider = |trial: Vec<usize>| {
            let s = candidate(p, &trial);
            if (s.cost, &s.assignment) < (best.cost, &best.assignment) {
                best = s;
                next = trial;
            }
        };
        for i in 0..a.len() {
            for j in 0..p.zones.len() {
                if a[i] != j
                    && counts[a[i]] > p.zones[a[i]].min
                    && counts[j] < p.zones[j].max
                    && p.guards[i].costs.contains_key(&p.zones[j].id)
                {
                    let mut trial = a.clone();
                    trial[i] = j;
                    consider(trial);
                }
            }
            for k in i + 1..a.len() {
                if a[i] != a[k]
                    && p.guards[i].costs.contains_key(&p.zones[a[k]].id)
                    && p.guards[k].costs.contains_key(&p.zones[a[i]].id)
                {
                    let mut trial = a.clone();
                    trial.swap(i, k);
                    consider(trial);
                }
            }
        }
        if best == current {
            break;
        }
        a = next;
        counts.fill(0);
        for &j in &a {
            counts[j] += 1;
        }
        current = best;
    }
    Some(current)
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 || args.len() > 4 {
        return Err("usage: idaptik-place-guards exact|heuristic INSTANCE.json; or validate INSTANCE.json SOLUTION.json".into());
    }
    let mut p: Problem =
        serde_json::from_slice(&std::fs::read(&args[2]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    p.prepare()?;
    if args[1] == "validate" {
        let path = args.get(3).ok_or("validate needs a solution file")?;
        let s: Solution = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        println!("{}", json!({"valid":true, "edit_script":validate(&p, &s)?}));
        return Ok(());
    }
    if args.len() != 3 {
        return Err("unexpected solution argument".into());
    }
    let solution = match args[1].as_str() {
        "exact" => exact(&p)?,
        "heuristic" => heuristic(&p),
        _ => return Err("unknown solver".into()),
    };
    let script = solution.as_ref().map(|s| validate(&p, s)).transpose()?;
    let status = if solution.is_some() {
        "feasible"
    } else if args[1] == "exact" {
        "infeasible"
    } else {
        "no_candidate"
    };
    println!(
        "{}",
        json!({"instance":p.id, "solver":args[1], "status":status,
        "solution":solution, "edit_script":script})
    );
    Ok(())
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => {
            // Linux benchmark opt-in: /proc resets VmHWM at exec, unlike
            // wait4 ru_maxrss which can retain the launcher high-water mark.
            if std::env::var_os("IDAPTIK_PLACEMENT_METRICS").is_some() {
                let status =
                    std::fs::read_to_string("/proc/self/status").expect("Linux procfs metrics");
                let line = status
                    .lines()
                    .find(|line| line.starts_with("VmHWM:"))
                    .expect("VmHWM");
                let kib: u64 = line.split_whitespace().nth(1).unwrap().parse().unwrap();
                eprintln!("{}", json!({"peak_rss_kib":kib}));
            }
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
