// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! The explain lens: one `enaction-runner-output/1` document and one
//! `enaction-mind-trace/1` document become a designer report.
//!
//! The mind trace is read exactly as `WEEVIL-HOST-ROUTE.adoc` ("The mind
//! trace contract") defines it, with unknown fields refused. A mind
//! publishes a ranking of candidates and never a score, so the lens shows
//! rankings and derives nothing from utilities.
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
//!   assert it; the lens derives it. Its per-agent count is in ticks;
//! * beliefs held by the agent, each beside the world-truth atoms for the
//!   same frame, marked `diverges-from-world` where they disagree.
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
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;

/// Contract tag of the report.
pub const REPORT_CONTRACT: &str = "ums-explain/1";
/// Contract tag of the mind trace this lens reads.
pub const MIND_TRACE_CONTRACT: &str = "enaction-mind-trace/1";
/// Contract tag of the runner output this lens reads.
pub const RUNNER_OUTPUT_CONTRACT: &str = "enaction-runner-output/1";
/// The only leak verdict this lens can give.
pub const LEAK_VERDICT: &str = "unverified (no counterfactual seam)";

const EVIDENCE: &str = "crates/ums-profile-sdk/tests/explain_golden.rs";

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
    attention: u64,
    caused_by: Vec<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Conation {
    ranking: Vec<String>,
    #[serde(default)]
    incumbent: Option<String>,
    selected: String,
    #[serde(default)]
    target: Option<String>,
}

fn guarantee(level: GuaranteeLevel, evidence: &str) -> Value {
    json!({"level": level, "evidence": evidence})
}

/// Pick the runner output (the document with `package` and `world_truth`)
/// and the mind trace (contract `enaction-mind-trace/1`) from a runner's
/// document stream, then explain them.
pub fn explain_documents(documents: &[Value], validated: bool) -> Result<Value, ExplainError> {
    let runner_output = documents
        .iter()
        .find(|document| document.get("package").is_some() && document.get("world_truth").is_some())
        .ok_or_else(|| {
            ExplainError::new(
                "MissingDocument",
                format!(
                    "no {RUNNER_OUTPUT_CONTRACT} document (an object with package and world_truth)"
                ),
            )
        })?;
    let mind_trace = documents
        .iter()
        .find(|document| {
            document.get("contract").and_then(Value::as_str) == Some(MIND_TRACE_CONTRACT)
        })
        .ok_or_else(|| {
            ExplainError::new(
                "MissingDocument",
                format!("no {MIND_TRACE_CONTRACT} document"),
            )
        })?;
    explain(runner_output, mind_trace, validated)
}

/// Explain one runner output and one mind trace. `validated` records
/// whether a validator process accepted these documents; the lens itself
/// does not validate them beyond what it needs to read them.
pub fn explain(
    runner_output: &Value,
    mind_trace: &Value,
    validated: bool,
) -> Result<Value, ExplainError> {
    canonical::check_integers(runner_output)?;
    canonical::check_integers(mind_trace)?;
    let trace: MindTrace = serde_json::from_value(mind_trace.clone())
        .map_err(|error| ExplainError::new("MalformedMindTrace", error.to_string()))?;
    if trace.contract != MIND_TRACE_CONTRACT {
        return Err(ExplainError::new(
            "WrongContract",
            format!("expected {MIND_TRACE_CONTRACT}, found {}", trace.contract),
        ));
    }
    let world = RunnerOutput::read(runner_output)?;

    let mut agents: BTreeMap<String, AgentRows> = BTreeMap::new();
    for record in &trace.ticks {
        agents
            .entry(record.agent.clone())
            .or_default()
            .push_tick(record, &world)?;
    }
    for belief in &world.beliefs {
        let (key, plain) = match belief.get("holder") {
            Some(Value::String(holder)) => (holder.clone(), true),
            Some(other) => (canonical::to_canonical_string(other)?, false),
            None => {
                return Err(ExplainError::new(
                    "MalformedRunnerOutput",
                    "a belief has no holder",
                ));
            }
        };
        let row = world.belief_row(belief, plain);
        agents.entry(key).or_default().beliefs.push(row);
    }

    let agents: Vec<Value> = agents
        .into_iter()
        .map(|(agent, rows)| rows.finish(agent))
        .collect();
    Ok(json!({
        "report": REPORT_CONTRACT,
        "inputs": {
            "runner_output": RUNNER_OUTPUT_CONTRACT,
            "mind_trace": MIND_TRACE_CONTRACT,
            "validation": if validated { "validator-accepted" } else { "not-validated" },
        },
        "producer": {"kernel": trace.producer.kernel, "abi": trace.producer.abi},
        "profile": {"id": trace.profile.id, "version": trace.profile.version},
        "leak_verdict": LEAK_VERDICT,
        "agents": agents,
    }))
}

struct RunnerOutput<'a> {
    events: BTreeMap<u64, &'a Value>,
    beliefs: Vec<&'a Value>,
    truth: BTreeMap<String, Vec<String>>,
}

impl<'a> RunnerOutput<'a> {
    fn read(document: &'a Value) -> Result<Self, ExplainError> {
        let malformed = |detail: &str| ExplainError::new("MalformedRunnerOutput", detail);
        let package = document
            .get("package")
            .and_then(Value::as_object)
            .ok_or_else(|| malformed("package is not an object"))?;
        let truth_atoms = document
            .get("world_truth")
            .and_then(Value::as_array)
            .ok_or_else(|| malformed("world_truth is not an array"))?;
        let array = |name: &str| -> Result<Vec<&'a Value>, ExplainError> {
            match package.get(name) {
                None => Ok(Vec::new()),
                Some(Value::Array(items)) => Ok(items.iter().collect()),
                Some(_) => Err(malformed(&format!("package.{name} is not an array"))),
            }
        };
        let mut events = BTreeMap::new();
        for event in array("events")? {
            if let Some(id) = event.get("event_id").and_then(Value::as_u64) {
                events.insert(id, event);
            }
        }
        let mut truth: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for atom in truth_atoms {
            let (Some(frame), Some(hypothesis)) = (
                atom.get("frame").and_then(Value::as_str),
                atom.get("hypothesis").and_then(Value::as_str),
            ) else {
                return Err(malformed("a world_truth atom lacks frame or hypothesis"));
            };
            let hypotheses = truth.entry(frame.to_owned()).or_default();
            hypotheses.push(hypothesis.to_owned());
            hypotheses.sort();
            hypotheses.dedup();
        }
        Ok(Self {
            events,
            beliefs: array("beliefs")?,
            truth,
        })
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

    fn belief_row(&self, belief: &Value, plain_holder: bool) -> Value {
        let proposition = belief.get("proposition").cloned().unwrap_or(Value::Null);
        let status = belief.get("status").and_then(Value::as_str).unwrap_or("");
        let frame = proposition.get("frame").and_then(Value::as_str);
        let hypothesis = proposition.get("hypothesis").and_then(Value::as_str);
        let truth = frame.and_then(|frame| self.truth.get(frame));
        let world = match (frame, hypothesis, truth) {
            (None, _, _) | (_, None, _) => "not-comparable",
            (Some(_), Some(_), None) => "no-world-truth",
            (Some(_), Some(hypothesis), Some(truth)) => {
                let holds = truth
                    .iter()
                    .any(|true_hypothesis| true_hypothesis == hypothesis);
                match status.to_ascii_lowercase().as_str() {
                    "known" | "certain" | "believed" | "suspected" => {
                        if holds {
                            "agrees-with-world"
                        } else {
                            "diverges-from-world"
                        }
                    }
                    "rejected" => {
                        if holds {
                            "diverges-from-world"
                        } else {
                            "agrees-with-world"
                        }
                    }
                    "unknown" => "no-claim",
                    _ => "not-comparable",
                }
            }
        };
        let level = if plain_holder {
            guarantee(GuaranteeLevel::Tested, EVIDENCE)
        } else {
            guarantee(
                GuaranteeLevel::Designed,
                "holder is not a plain agent id (nested belief); grouping it under its canonical JSON is not covered by a test of nested semantics",
            )
        };
        json!({
            "belief_id": belief.get("belief_id").cloned().unwrap_or(Value::Null),
            "proposition": proposition,
            "status": status,
            "source_events": belief.get("source_events").cloned().unwrap_or(json!([])),
            "world_truth": truth.cloned().map_or(Value::Null, |truth| json!(truth)),
            "world": world,
            "guarantee": level,
        })
    }
}

#[derive(Default)]
struct AgentRows {
    windows: Vec<Map<String, Value>>,
    last_key: Option<Value>,
    last_token: Option<String>,
    override_ticks: u64,
    ticks: u64,
    beliefs: Vec<Value>,
}

impl AgentRows {
    fn push_tick(&mut self, record: &TickRecord, world: &RunnerOutput) -> Result<(), ExplainError> {
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
        let override_tick =
            conation.incumbent.as_deref() == Some(conation.selected.as_str()) && rank != 0;
        self.ticks += 1;
        if override_tick {
            self.override_ticks += 1;
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
                    let low = affect["attention_min"].as_u64().unwrap_or(u64::MAX);
                    let high = affect["attention_max"].as_u64().unwrap_or(0);
                    affect.insert(
                        "attention_min".into(),
                        json!(low.min(record.affect.attention)),
                    );
                    affect.insert(
                        "attention_max".into(),
                        json!(high.max(record.affect.attention)),
                    );
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
            "commitment_override_ticks": self.override_ticks,
            "commitment_override_guarantee": guarantee(GuaranteeLevel::Tested, EVIDENCE),
            "windows": self.windows,
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

/// Plain-text rendering of a report from [`explain`].
pub fn render_text(report: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "explain report {}", text(&report["report"]));
    let _ = writeln!(
        out,
        "inputs: {} + {} ({})",
        text(&report["inputs"]["runner_output"]),
        text(&report["inputs"]["mind_trace"]),
        text(&report["inputs"]["validation"])
    );
    let _ = writeln!(
        out,
        "profile: {} {}; producer: {} abi {}",
        text(&report["profile"]["id"]),
        text(&report["profile"]["version"]),
        text(&report["producer"]["kernel"]),
        text(&report["producer"]["abi"])
    );
    let _ = writeln!(out, "leak-verdict: {}", text(&report["leak_verdict"]));
    for agent in report["agents"].as_array().into_iter().flatten() {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "agent {}: commitment-override ticks: {} of {} [{}]",
            text(&agent["agent"]),
            agent["commitment_override_ticks"],
            agent["ticks"],
            text(&agent["commitment_override_guarantee"]["level"])
        );
        for window in agent["windows"].as_array().into_iter().flatten() {
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
                format!("{}", affect["attention_min"])
            } else {
                format!("{}..{}", affect["attention_min"], affect["attention_max"])
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
        for belief in agent["beliefs"].as_array().into_iter().flatten() {
            let proposition = &belief["proposition"];
            let truth = match &belief["world_truth"] {
                Value::Null => "none".to_owned(),
                truth => format!("{}={}", text(&proposition["frame"]), words(truth, "|")),
            };
            let _ = writeln!(
                out,
                "  belief {}: {}={} ({}); world: {} -> {} [{}]",
                text(&belief["belief_id"]),
                text(&proposition["frame"]),
                text(&proposition["hypothesis"]),
                text(&belief["status"]),
                truth,
                text(&belief["world"]),
                level(belief)
            );
        }
    }
    out
}
