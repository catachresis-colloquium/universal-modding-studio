// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! The explain lens: one `enaction-runner-output/1` document and, when the
//! runner emitted one, one `enaction-mind-trace/1` document become a
//! designer report.
//!
//! Both documents are read in the shapes Enaction defines (CAC-KERNEL
//! sections 9 and 10) and that `conformance/mind/v1` pins, with unknown
//! fields refused. A mind publishes a ranking of candidates and never a
//! score, so the lens shows rankings and derives nothing from utilities.
//!
//! Per agent the report has:
//!
//! * tick windows: maximal runs of the agent's consecutive trace records
//!   with the same affect token, ranking, incumbent, selected candidate,
//!   target and tells. Each window shows the token, whether it changed and
//!   which events caused the change, the ranking, the incumbent, the
//!   selected candidate and its rank, and the tells;
//! * the derived flag `commitment-override`: the selected candidate equals
//!   the incumbent and is not first in the ranking. The producer does not
//!   assert it; the lens derives it. Its ticks are listed per agent;
//! * the ticks at which each tell is shown;
//! * the beliefs the agent bears, each compared with world truth under the
//!   closed-world rule of CAC-KERNEL section 9: world truth lists every
//!   stated proposition that is true, so a stated proposition absent from
//!   it is false. A held belief (`certain`, `believed`, `suspected`) agrees
//!   when its proposition is present and diverges when it is absent; a
//!   `rejected` belief diverges when its proposition is present and agrees
//!   otherwise; `unknown` makes no claim.
//!
//! Every derived row carries a [`GuaranteeLevel`]: `tested` where a test in
//! `tests/explain_golden.rs` covers the derivation, `designed` otherwise.
//!
//! The leak verdict is out of scope: it needs the counterfactual seam
//! (completion-plan step 22), so the report always says
//! [`LEAK_VERDICT`] and never `no-leak`.

use crate::GuaranteeLevel;
use crate::canonical::{self, CanonicalError};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Write as _;

/// Contract tag of the report.
pub const REPORT_CONTRACT: &str = "ums-explain/2";
/// Contract tag of the mind trace this lens reads.
pub const MIND_TRACE_CONTRACT: &str = "enaction-mind-trace/1";
/// Contract tag of the runner output this lens reads.
pub const RUNNER_OUTPUT_CONTRACT: &str = "enaction-runner-output/1";
/// The only leak verdict this lens can give.
pub const LEAK_VERDICT: &str = "unverified (no counterfactual seam)";

const EVIDENCE: &str = "crates/ums-profile-sdk/tests/explain_golden.rs";
const AGREES: &str = "agrees-with-world";
const DIVERGES: &str = "diverges-from-world";
const NO_CLAIM: &str = "no-claim";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplainError {
    rule: &'static str,
    detail: String,
}

impl ExplainError {
    fn new(rule: &'static str, detail: impl Into<String>) -> Self {
        Self {
            rule,
            detail: detail.into(),
        }
    }

    /// Stable rule name of the refusal.
    pub fn rule(&self) -> &'static str {
        self.rule
    }
}

impl fmt::Display for ExplainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.rule, self.detail)
    }
}

impl std::error::Error for ExplainError {}

impl From<CanonicalError> for ExplainError {
    fn from(error: CanonicalError) -> Self {
        let rule = match error {
            CanonicalError::NonInteger { .. } => "NonInteger",
            CanonicalError::IntegerOutOfRange { .. } => "IntegerOutOfRange",
        };
        Self::new(rule, error.to_string())
    }
}

// ---- enaction-mind-trace/1 (CAC-KERNEL section 10) ----

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MindTrace {
    contract: String,
    producer: Producer,
    profile: ProfileRef,
    #[allow(dead_code)] // Vocabulary membership is the validator's rule.
    vocabulary: Vocabulary,
    ticks: Vec<TickRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Producer {
    kernel: String,
    abi: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileRef {
    id: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct Vocabulary {
    affect_tokens: Vec<String>,
    candidates: Vec<String>,
    tells: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TickRecord {
    tick: u64,
    agent: String,
    affect: Affect,
    conation: Conation,
    tells: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Affect {
    token: String,
    #[serde(default)]
    attention: Option<u64>,
    caused_by: Vec<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Conation {
    ranking: Vec<String>,
    incumbent: String,
    selected: String,
    #[serde(default)]
    target: Option<String>,
}

// ---- enaction-runner-output/1 (CAC-KERNEL section 9) ----

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    contract: String,
    package: Package,
    world_truth: Vec<String>,
}

/// The members of `EpistemicPackage` the lens reads. Events and the
/// manifest are read loosely: their full rules are the validator's.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    #[serde(default)]
    events: Vec<Value>,
    #[serde(default)]
    beliefs: Vec<Belief>,
    manifest: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Belief {
    belief_id: u64,
    #[serde(default)]
    holders: Vec<String>,
    bearer: String,
    proposition: String,
    status: Status,
    confidence: Interval,
    source_events: Vec<u64>,
    valid_from: u64,
    #[serde(default)]
    supersedes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Status {
    Certain,
    Believed,
    Suspected,
    Rejected,
    Unknown,
}

impl Status {
    fn name(self) -> &'static str {
        match self {
            Self::Certain => "certain",
            Self::Believed => "believed",
            Self::Suspected => "suspected",
            Self::Rejected => "rejected",
            Self::Unknown => "unknown",
        }
    }

    /// The closed-world comparison (CAC-KERNEL section 9).
    fn world(self, in_world_truth: bool) -> &'static str {
        match (self, in_world_truth) {
            (Self::Unknown, _) => NO_CLAIM,
            (Self::Rejected, true) => DIVERGES,
            (Self::Rejected, false) => AGREES,
            (_, true) => AGREES,
            (_, false) => DIVERGES,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Interval {
    belief: u64,
    plausibility: u64,
}

fn guarantee(level: GuaranteeLevel, evidence: &str) -> Value {
    json!({"level": level, "evidence": evidence})
}

/// Explain one runner output and, optionally, one mind trace. `validated`
/// records whether a validator process accepted these documents; the lens
/// itself does not validate them beyond what it needs to read them.
pub fn explain(
    runner_output: &Value,
    mind_trace: Option<&Value>,
    validated: bool,
) -> Result<Value, ExplainError> {
    canonical::check_integers(runner_output)?;
    let envelope: Envelope = serde_json::from_value(runner_output.clone())
        .map_err(|error| ExplainError::new("MalformedRunnerOutput", error.to_string()))?;
    if envelope.contract != RUNNER_OUTPUT_CONTRACT {
        return Err(ExplainError::new(
            "WrongContract",
            format!(
                "expected {RUNNER_OUTPUT_CONTRACT}, found {}",
                envelope.contract
            ),
        ));
    }
    let trace = match mind_trace {
        None => None,
        Some(document) => {
            canonical::check_integers(document)?;
            let trace: MindTrace = serde_json::from_value(document.clone())
                .map_err(|error| ExplainError::new("MalformedMindTrace", error.to_string()))?;
            if trace.contract != MIND_TRACE_CONTRACT {
                return Err(ExplainError::new(
                    "WrongContract",
                    format!("expected {MIND_TRACE_CONTRACT}, found {}", trace.contract),
                ));
            }
            Some(trace)
        }
    };
    let world = World::read(&envelope);

    let mut agents: BTreeMap<String, AgentRows> = BTreeMap::new();
    for record in trace.iter().flat_map(|trace| &trace.ticks) {
        agents
            .entry(record.agent.clone())
            .or_default()
            .push_tick(record, &world)?;
    }
    let mut overall = Summary::default();
    for belief in &envelope.package.beliefs {
        let row = world.belief_row(belief);
        overall.count(&row);
        let rows = agents.entry(belief.bearer.clone()).or_default();
        rows.summary.count(&row);
        rows.beliefs.push(row);
    }

    let agents: Vec<Value> = agents
        .into_iter()
        .map(|(agent, rows)| rows.finish(agent))
        .collect();
    let manifest_profile = &envelope.package.manifest["profile"];
    let package_profile = json!({
        "id": manifest_profile["id"],
        "version": format!(
            "{}.{}",
            text(&manifest_profile["version"]["major"]),
            text(&manifest_profile["version"]["minor"])
        ),
    });
    Ok(json!({
        "report": REPORT_CONTRACT,
        "inputs": {
            "runner_output": RUNNER_OUTPUT_CONTRACT,
            "mind_trace": if trace.is_some() { MIND_TRACE_CONTRACT } else { "absent" },
            "validation": if validated { "validator-accepted" } else { "not-validated" },
        },
        "producer": trace.as_ref().map(|trace| json!({"kernel": trace.producer.kernel, "abi": trace.producer.abi})),
        "profile": trace.as_ref().map(|trace| json!({"id": trace.profile.id, "version": trace.profile.version})),
        "package_profile": package_profile,
        "leak_verdict": LEAK_VERDICT,
        "belief_summary": overall.to_value(),
        "agents": agents,
    }))
}

struct World<'a> {
    events: BTreeMap<u64, &'a Value>,
    truth: BTreeSet<&'a str>,
}

impl<'a> World<'a> {
    fn read(envelope: &'a Envelope) -> Self {
        let mut events = BTreeMap::new();
        for event in &envelope.package.events {
            if let Some(id) = event.get("event_id").and_then(Value::as_u64) {
                events.insert(id, event);
            }
        }
        Self {
            events,
            truth: envelope.world_truth.iter().map(String::as_str).collect(),
        }
    }

    fn cause(&self, id: u64) -> Value {
        let mut out = Map::new();
        out.insert("id".into(), json!(id));
        match self.events.get(&id) {
            Some(event) => {
                out.insert("resolved".into(), json!(true));
                for name in ["kind", "tick"] {
                    if let Some(value) = event.get(name) {
                        out.insert(name.into(), value.clone());
                    }
                }
            }
            None => {
                out.insert("resolved".into(), json!(false));
            }
        }
        Value::Object(out)
    }

    fn belief_row(&self, belief: &Belief) -> Value {
        let in_world_truth = self.truth.contains(belief.proposition.as_str());
        let level = if belief.holders.is_empty() {
            guarantee(GuaranteeLevel::Tested, EVIDENCE)
        } else {
            guarantee(
                GuaranteeLevel::Designed,
                "nested belief (holders is not empty): the closed-world comparison is applied to its proposition, but no test covers the semantics of comparing a modelled belief with the world",
            )
        };
        json!({
            "belief_id": belief.belief_id,
            "bearer": belief.bearer,
            "holders": belief.holders,
            "proposition": belief.proposition,
            "status": belief.status.name(),
            "confidence": {
                "belief": belief.confidence.belief,
                "plausibility": belief.confidence.plausibility,
            },
            "source_events": belief.source_events,
            "valid_from": belief.valid_from,
            "supersedes": belief.supersedes,
            "in_world_truth": in_world_truth,
            "world": belief.status.world(in_world_truth),
            "guarantee": level,
        })
    }
}

#[derive(Default)]
struct Summary {
    beliefs: u64,
    agrees: u64,
    diverges: u64,
    no_claim: u64,
}

impl Summary {
    fn count(&mut self, row: &Value) {
        self.beliefs += 1;
        match row["world"].as_str() {
            Some(AGREES) => self.agrees += 1,
            Some(DIVERGES) => self.diverges += 1,
            _ => self.no_claim += 1,
        }
    }

    fn to_value(&self) -> Value {
        json!({
            "beliefs": self.beliefs,
            AGREES: self.agrees,
            DIVERGES: self.diverges,
            NO_CLAIM: self.no_claim,
        })
    }
}

#[derive(Default)]
struct AgentRows {
    windows: Vec<Map<String, Value>>,
    last_key: Option<Value>,
    last_token: Option<String>,
    override_at: Vec<u64>,
    tells_at: BTreeMap<String, Vec<u64>>,
    ticks: u64,
    beliefs: Vec<Value>,
    summary: Summary,
}

fn attention_bound(current: &Value, next: Option<u64>, pick: fn(u64, u64) -> u64) -> Value {
    match (current.as_u64(), next) {
        (Some(a), Some(b)) => json!(pick(a, b)),
        (Some(a), None) => json!(a),
        (None, next) => json!(next),
    }
}

impl AgentRows {
    fn push_tick(&mut self, record: &TickRecord, world: &World) -> Result<(), ExplainError> {
        let conation = &record.conation;
        let rank = conation
            .ranking
            .iter()
            .position(|candidate| candidate == &conation.selected)
            .ok_or_else(|| {
                ExplainError::new(
                    "SelectedNotInRanking",
                    format!(
                        "agent {} tick {}: selected {} is not in the ranking",
                        record.agent, record.tick, conation.selected
                    ),
                )
            })?;
        let override_tick = conation.incumbent == conation.selected && rank != 0;
        self.ticks += 1;
        if override_tick {
            self.override_at.push(record.tick);
        }
        for tell in &record.tells {
            self.tells_at
                .entry(tell.clone())
                .or_default()
                .push(record.tick);
        }

        let key = json!([
            record.affect.token,
            conation.ranking,
            conation.incumbent,
            conation.selected,
            conation.target,
            record.tells,
        ]);
        if self.last_key.as_ref() == Some(&key) {
            if let Some(window) = self.windows.last_mut() {
                window.insert("last_tick".into(), json!(record.tick));
                let count = window["ticks"].as_u64().unwrap_or(0) + 1;
                window.insert("ticks".into(), json!(count));
                if let Some(Value::Object(affect)) = window.get_mut("affect") {
                    let low = attention_bound(
                        &affect["attention_min"],
                        record.affect.attention,
                        u64::min,
                    );
                    let high = attention_bound(
                        &affect["attention_max"],
                        record.affect.attention,
                        u64::max,
                    );
                    affect.insert("attention_min".into(), low);
                    affect.insert("attention_max".into(), high);
                }
            }
            return Ok(());
        }

        let change = match &self.last_token {
            None => "initial",
            Some(previous) if previous != &record.affect.token => "changed",
            Some(_) => "unchanged",
        };
        let causes: Vec<Value> = record
            .affect
            .caused_by
            .iter()
            .map(|id| world.cause(*id))
            .collect();
        let window = json!({
            "first_tick": record.tick,
            "last_tick": record.tick,
            "ticks": 1,
            "affect": {
                "token": record.affect.token,
                "change": change,
                "previous": self.last_token,
                "caused_by": causes,
                "attention_min": record.affect.attention,
                "attention_max": record.affect.attention,
            },
            "ranking": conation.ranking,
            "incumbent": conation.incumbent,
            "selected": conation.selected,
            "selected_rank": rank + 1,
            "target": conation.target,
            "tells": record.tells,
            "commitment_override": override_tick,
            "guarantee": guarantee(GuaranteeLevel::Tested, EVIDENCE),
        });
        if let Value::Object(window) = window {
            self.windows.push(window);
        }
        self.last_key = Some(key);
        self.last_token = Some(record.affect.token.clone());
        Ok(())
    }

    fn finish(self, agent: String) -> Value {
        json!({
            "agent": agent,
            "ticks": self.ticks,
            "commitment_override_ticks": self.override_at.len(),
            "commitment_override_at": self.override_at,
            "commitment_override_guarantee": guarantee(GuaranteeLevel::Tested, EVIDENCE),
            "tells_at": self.tells_at,
            "windows": self.windows,
            "belief_summary": self.summary.to_value(),
            "beliefs": self.beliefs,
        })
    }
}

fn level(row: &Value) -> &str {
    row["guarantee"]["level"].as_str().unwrap_or("?")
}

fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "none".into(),
        other => canonical::to_canonical_string(other).unwrap_or_else(|_| other.to_string()),
    }
}

fn words(values: &Value, separator: &str) -> String {
    values
        .as_array()
        .map(|items| items.iter().map(text).collect::<Vec<_>>().join(separator))
        .unwrap_or_default()
}

/// Ticks as comma-separated runs: `[3, 4, 5, 9]` is `3..5, 9`.
fn tick_ranges(ticks: &Value) -> String {
    let ticks: Vec<u64> = ticks
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .collect();
    let mut runs: Vec<(u64, u64)> = Vec::new();
    for tick in ticks {
        match runs.last_mut() {
            Some((_, end)) if tick == *end + 1 => *end = tick,
            _ => runs.push((tick, tick)),
        }
    }
    runs.iter()
        .map(|(start, end)| {
            if start == end {
                start.to_string()
            } else {
                format!("{start}..{end}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn summary_line(summary: &Value) -> String {
    format!(
        "beliefs: {}; {AGREES}: {}; {DIVERGES}: {}; {NO_CLAIM}: {}",
        summary["beliefs"], summary[AGREES], summary[DIVERGES], summary[NO_CLAIM]
    )
}

/// Plain-text rendering of a report from [`explain`].
pub fn render_text(report: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "explain report {}", text(&report["report"]));
    let mind = match report["inputs"]["mind_trace"].as_str() {
        Some("absent") => "no mind trace".to_owned(),
        _ => text(&report["inputs"]["mind_trace"]),
    };
    let _ = writeln!(
        out,
        "inputs: {} + {} ({})",
        text(&report["inputs"]["runner_output"]),
        mind,
        text(&report["inputs"]["validation"])
    );
    let _ = writeln!(
        out,
        "package profile: {} {}",
        text(&report["package_profile"]["id"]),
        text(&report["package_profile"]["version"])
    );
    if !report["profile"].is_null() {
        let _ = writeln!(
            out,
            "mind profile: {} {}; producer: {} abi {}",
            text(&report["profile"]["id"]),
            text(&report["profile"]["version"]),
            text(&report["producer"]["kernel"]),
            text(&report["producer"]["abi"])
        );
    }
    let _ = writeln!(out, "leak-verdict: {}", text(&report["leak_verdict"]));
    let _ = writeln!(out, "{}", summary_line(&report["belief_summary"]));
    for agent in report["agents"].as_array().into_iter().flatten() {
        let _ = writeln!(out);
        let overrides = tick_ranges(&agent["commitment_override_at"]);
        let _ = writeln!(
            out,
            "agent {}: commitment-override ticks: {} of {}{} [{}]",
            text(&agent["agent"]),
            agent["commitment_override_ticks"],
            agent["ticks"],
            if overrides.is_empty() {
                String::new()
            } else {
                format!(" ({overrides})")
            },
            text(&agent["commitment_override_guarantee"]["level"])
        );
        let tells: Vec<String> = agent["tells_at"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(tell, ticks)| format!("{tell}@{}", tick_ranges(ticks)))
            .collect();
        if !tells.is_empty() {
            let _ = writeln!(out, "  tells at: {}", tells.join("; "));
        }
        let _ = writeln!(out, "  {}", summary_line(&agent["belief_summary"]));
        for window in agent["windows"].as_array().into_iter().flatten() {
            render_window(&mut out, window);
        }
        for belief in agent["beliefs"].as_array().into_iter().flatten() {
            let holders = words(&belief["holders"], " > ");
            let _ = writeln!(
                out,
                "  belief {}: {}{:?} ({} {}..{}); {} world truth -> {} [{}]",
                text(&belief["belief_id"]),
                if holders.is_empty() {
                    String::new()
                } else {
                    format!("as modelled by {holders}: ")
                },
                text(&belief["proposition"]),
                text(&belief["status"]),
                belief["confidence"]["belief"],
                belief["confidence"]["plausibility"],
                if belief["in_world_truth"] == true {
                    "in"
                } else {
                    "absent from"
                },
                text(&belief["world"]),
                level(belief)
            );
        }
    }
    out
}

fn render_window(out: &mut String, window: &Value) {
    let ticks = window["ticks"].as_u64().unwrap_or(0);
    let _ = writeln!(
        out,
        "  ticks {}..{} ({} tick{})",
        window["first_tick"],
        window["last_tick"],
        ticks,
        if ticks == 1 { "" } else { "s" }
    );
    let affect = &window["affect"];
    let mut change = match affect["change"].as_str() {
        Some("changed") => format!("changed from {}", text(&affect["previous"])),
        Some(other) => other.to_owned(),
        None => String::new(),
    };
    let causes: Vec<String> = affect["caused_by"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|cause| {
            if cause["resolved"] == true {
                format!(
                    "#{} {}@{}",
                    cause["id"],
                    text(&cause["kind"]),
                    text(&cause["tick"])
                )
            } else {
                format!("#{} unresolved", cause["id"])
            }
        })
        .collect();
    if !causes.is_empty() {
        let _ = write!(change, "; caused by {}", causes.join(", "));
    }
    let attention = if affect["attention_min"] == affect["attention_max"] {
        text(&affect["attention_min"])
    } else {
        format!(
            "{}..{}",
            text(&affect["attention_min"]),
            text(&affect["attention_max"])
        )
    };
    let _ = writeln!(
        out,
        "    affect: {} ({}); attention {}",
        text(&affect["token"]),
        change,
        attention
    );
    let _ = writeln!(
        out,
        "    ranking: {}; incumbent: {}; selected: {} (rank {}); target: {}",
        words(&window["ranking"], " > "),
        text(&window["incumbent"]),
        text(&window["selected"]),
        window["selected_rank"],
        text(&window["target"])
    );
    let tells = words(&window["tells"], ", ");
    let _ = writeln!(
        out,
        "    tells: {}",
        if tells.is_empty() {
            "none".into()
        } else {
            tells
        }
    );
    if window["commitment_override"] == true {
        let _ = writeln!(out, "    commitment-override [{}]", level(window));
    }
}
