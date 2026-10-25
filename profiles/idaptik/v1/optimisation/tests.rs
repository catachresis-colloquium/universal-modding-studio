// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
use super::*;

fn problem(costs: &[&[u32]], min: usize, max: usize) -> Problem {
    let n = costs.first().map_or(2, |r| r.len());
    let value = json!({
        "format":"idaptik-guard-placement/v1", "id":"test",
        "base_script":{"profile":"idaptik", "edits":(0..n).map(|j| json!({
            "verb":"add_zone", "id":format!("z{j}"), "securityTier":0,
            "worldXStart":j * 100, "worldXEnd":(j+1)*100})).collect::<Vec<_>>()},
        "zones":(0..n).map(|j|json!({"id":format!("z{j}"),"min":min,"max":max})).collect::<Vec<_>>(),
        "guards":costs.iter().enumerate().map(|(i,c)|json!({"id":format!("g{i}"),
            "rank":"BasicGuard", "costs":c.iter().enumerate().map(|(j,&c)|(format!("z{j}"),c)).collect::<BTreeMap<_,_>>()})).collect::<Vec<_>>()
    });
    let mut p: Problem = serde_json::from_value(value).unwrap();
    p.prepare().unwrap();
    p
}

#[test]
fn exact_ties_and_input_order_are_canonical() {
    let mut p = problem(&[&[1, 1], &[1, 1]], 1, 1);
    let s = exact(&p).unwrap().unwrap();
    assert_eq!(s.cost, 2);
    assert_eq!(s.assignment["g0"], "z0");
    p.guards.reverse();
    p.zones.reverse();
    p.prepare().unwrap();
    assert_eq!(exact(&p).unwrap().unwrap(), s);
    assert_eq!(heuristic(&p).unwrap(), s);
}

#[test]
fn validator_rejects_corrupt_candidates() {
    let p = problem(&[&[1, 9], &[8, 2]], 1, 1);
    let good = exact(&p).unwrap().unwrap();
    assert!(validate(&p, &good).is_ok());
    let mut bad = good.clone();
    bad.cost += 1;
    assert!(validate(&p, &bad).is_err());
    let mut bad = good.clone();
    bad.assignment.remove("g0");
    assert!(validate(&p, &bad).is_err());
    let mut bad = good.clone();
    bad.assignment.insert("extra".into(), "z0".into());
    assert!(validate(&p, &bad).is_err());
    let mut bad = good.clone();
    bad.assignment.insert("g0".into(), "unknown".into());
    assert!(validate(&p, &bad).is_err());
    let mut bad = good;
    bad.assignment.insert("g0".into(), "z1".into());
    bad.cost = 11;
    assert!(validate(&p, &bad).is_err());
}

#[test]
fn invalid_input_is_distinct_from_infeasible() {
    let mut p = problem(&[&[1, 2]], 1, 1);
    assert!(exact(&p).unwrap().is_none());
    assert!(heuristic(&p).is_none());
    p.guards[0].rank = "MadeUp".into();
    assert!(p.prepare().is_err());
    let mut p = problem(&[&[1, 2]], 0, 1);
    p.zones.push(p.zones[0].clone());
    assert!(p.prepare().is_err());
    let mut p = problem(&[&[1, 2]], 0, 1);
    p.guards[0].costs.insert("absent".into(), 0);
    assert!(p.prepare().is_err());
    let mut p = problem(&[&[1, 2]], 0, 1);
    p.base_script["edits"] = json!([]);
    assert!(p.prepare().is_err());
}

#[test]
fn empty_guards_zero_cost_and_solver_bound() {
    let p = problem(&[], 0, 1);
    assert_eq!(exact(&p).unwrap().unwrap().cost, 0);
    let p = problem(&[&[0, 0], &[0, 0]], 0, 2);
    assert_eq!(heuristic(&p).unwrap().cost, 0);
    let p = problem(&[&[1, 2][..]; 13], 0, 13);
    assert!(exact(&p).is_err());
    let p = problem(&[&[u32::MAX, u32::MAX][..]; 2], 0, 2);
    assert_eq!(exact(&p).unwrap().unwrap().cost, 2 * u64::from(u32::MAX));
}

#[test]
fn swap_improves_greedy_and_three_cycle_exposes_quality_gap() {
    let p = problem(&[&[1, 2], &[2, 100]], 1, 1);
    assert_eq!(heuristic(&p).unwrap().cost, 4);
    // No improving pair swap; a three-way rotation reduces cost from 6 to 5.
    let p = problem(&[&[2, 3, 100], &[100, 2, 2], &[0, 100, 2]], 1, 1);
    assert_eq!(heuristic(&p).unwrap().cost, 6);
    assert_eq!(exact(&p).unwrap().unwrap().cost, 5);
}

#[test]
fn exact_matches_independent_base_n_oracle_on_256_matrices() {
    for bits in 0..256 {
        let rows: Vec<Vec<u32>> = (0..4)
            .map(|i| (0..2).map(|j| ((bits >> (2 * i + j)) & 1) as u32).collect())
            .collect();
        let refs: Vec<&[u32]> = rows.iter().map(Vec::as_slice).collect();
        let p = problem(&refs, 2, 2);
        let expected = (0usize..16)
            .filter(|v| v.count_ones() == 2)
            .map(|v| {
                let cost: u64 = (0..4).map(|i| u64::from(rows[i][(v >> i) & 1])).sum();
                let assignment = (0..4)
                    .map(|i| (format!("g{i}"), format!("z{}", (v >> i) & 1)))
                    .collect::<BTreeMap<_, _>>();
                (cost, assignment)
            })
            .min()
            .unwrap();
        let s = exact(&p).unwrap().unwrap();
        assert_eq!((s.cost, s.assignment.clone()), expected);
        validate(&p, &s).unwrap();
        let h = heuristic(&p).unwrap();
        validate(&p, &h).unwrap();
        assert!(h.cost >= s.cost);
    }
}

#[test]
fn corpus_replays_and_heuristic_failure_is_not_infeasibility() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../profiles/idaptik/v1/optimisation/corpus");
    for entry in std::fs::read_dir(directory).unwrap() {
        let mut p: Problem =
            serde_json::from_slice(&std::fs::read(entry.unwrap().path()).unwrap()).unwrap();
        p.prepare().unwrap();
        let h = heuristic(&p);
        if let Some(ref s) = h {
            validate(&p, s).unwrap();
        }
        if p.guards.len() <= 12 {
            let e = exact(&p).unwrap();
            if let Some(ref s) = e {
                validate(&p, s).unwrap();
            }
            if let (Some(e), Some(h)) = (&e, &h) {
                assert!(h.cost >= e.cost);
            }
            if p.id == "greedy-dead-end" {
                assert!(e.is_some());
                assert!(h.is_none());
            }
        }
    }
}
