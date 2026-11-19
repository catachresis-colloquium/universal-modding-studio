// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! The explain lens: one `enaction-runner-output/2` document and, when the
//! runner emitted one, one `enaction-mind-trace/1` document become a
//! designer report.
//!
//! Both documents are read in the shapes Enaction defines for kernel
//! contract 2.0 (CAC-KERNEL sections 9 and 10, enaction-engine ADR-0030, a
//! draft) and that `conformance/tom/v2` and `conformance/mind/v2` pin, with
//! unknown fields refused. The lens reads the runner output's `contract`
//! tag before it decodes the body, and refuses any tag other than
//! [`RUNNER_OUTPUT_CONTRACT`] for that alone, with one finding
//! (`WrongContract` at `contract`): an `enaction-runner-output/1` document
//! is never reported as a malformed `/2` one. A mind publishes a ranking of candidates and never a
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
//! * the order-0 beliefs the agent bears, each with the belief it
//!   supersedes and the cause the link names (the event's id, kind,
//!   observer, tick and proposition), and each compared with world truth
//!   under the closed-world rule of CAC-KERNEL section 9: world truth lists
//!   every stated proposition that is true, so a stated proposition absent
//!   from it is false. A held belief (`certain`, `believed`, `suspected`)
//!   agrees when its proposition is present and diverges when it is
//!   absent; a `rejected` belief diverges when its proposition is present
//!   and agrees otherwise; `unknown` makes no claim.
//!
//! Per model (a belief of order 1 or more, `holders` not empty) the report
//! has a row in `models`, derived by the rule of CAC-KERNEL section 9,
//! "Models are compared with minds" (ADR-0028), and never compared with
//! world truth: the holder path, bearer, proposition and status, the
//! belief it supersedes with the cause the link names, the cited causes, the beliefs it is compared with (the outermost holder removed,
//! the same bearer and proposition) with their own order-0 world verdict,
//! whether the world holds the proposition (shown as a separate fact), and
//! the timeline over the model's lifetime: segments of `agrees`,
//! `diverges` or `no-claim`, and the derived mark `stale_divergent` (the
//! first segment agrees and a later one diverges). A model whose compared
//! belief is not unique inside its lifetime is shown as refused
//! (`model-counterpart-unique`) with no timeline. [`model_timelines`]
//! gives the same derivation in the shape of Enaction's
//! `conformance/tom/v2` `expected.json`, which `tests/vendored_tom.rs`
//! checks case by case.
//!
//! [`check_depth`] and [`check_links`] repeat the package rules that the
//! rejected cases of `conformance/tom/v2` name (`nesting-bounded`,
//! `belief-key-chained` and the six `supersession-cause-*` rules), so that
//! a reader of the corpus refuses what it refuses without running Enaction
//! code. They are not the validator: [`explain`] does not run them, and a
//! preview relies on `enaction-validate` for every package rule.
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
pub const REPORT_CONTRACT: &str = "ums-explain/3";
/// Contract tag of the mind trace this lens reads.
pub const MIND_TRACE_CONTRACT: &str = "enaction-mind-trace/1";
/// Contract tag of the runner output this lens reads.
pub const RUNNER_OUTPUT_CONTRACT: &str = "enaction-runner-output/2";
/// The only leak verdict this lens can give.
pub const LEAK_VERDICT: &str = "unverified (no counterfactual seam)";

const EVIDENCE: &str = "crates/ums-profile-sdk/tests/explain_golden.rs";
const MODEL_EVIDENCE: &str = "crates/ums-profile-sdk/tests/vendored_tom.rs";
const AGREES: &str = "agrees-with-world";
const DIVERGES: &str = "diverges-from-world";
const NO_CLAIM: &str = "no-claim";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplainError {
    rule: &'static str,
    detail: String,
    path: Option<String>,
}

impl ExplainError {
    fn new(rule: &'static str, detail: impl Into<String>) -> Self {
        Self {
            rule,
            detail: detail.into(),
            path: None,
        }
    }

    /// Stable rule name of the refusal.
    pub fn rule(&self) -> &'static str {
        self.rule
    }

    /// Field path of the refusal, where the rule names one.
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
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

// ---- enaction-runner-output/2 (CAC-KERNEL section 9) ----

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    #[allow(dead_code)] // `check_tag` reads the tag before the body is decoded.
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
    supersedes: Option<Link>,
}

/// A supersession link of kernel contract 2.0 (ADR-0030): the belief
/// replaced and the event that caused the revision. A link with no cause
/// has no wire form.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    belief: u64,
    because: u64,
}

impl Belief {
    /// The id of the belief this one replaces, if any.
    fn replaces(&self) -> Option<u64> {
        self.supersedes.map(|link| link.belief)
    }
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
    /// A belief holds its proposition when it is certain, believed or
    /// suspected (CAC-KERNEL section 9).
    fn holds(self) -> bool {
        matches!(self, Self::Certain | Self::Believed | Self::Suspected)
    }

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
    let envelope = read_envelope(runner_output)?;
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
    // World truth applies to order 0 only (CAC-KERNEL section 9). A model
    // is reported in `models`, compared with a mind, never here.
    for belief in envelope
        .package
        .beliefs
        .iter()
        .filter(|b| b.holders.is_empty())
    {
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
    let ledger = Ledger::new(&envelope.package.beliefs);
    let models: Vec<Value> = ledger
        .models()
        .into_iter()
        .map(|model| ledger.model_row(model, &world))
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
        "model_summary": model_summary(&models),
        "models": models,
    }))
}

/// Refuse a runner output whose `contract` tag is a string other than
/// [`RUNNER_OUTPUT_CONTRACT`], before anything else in it is read
/// (CAC-KERNEL section 9, "The contract tag"). A document with no
/// `contract` member, or one that is not a string, is left to the decoder.
fn check_tag(runner_output: &Value) -> Result<(), ExplainError> {
    match runner_output.get("contract") {
        Some(Value::String(tag)) if tag != RUNNER_OUTPUT_CONTRACT => {
            let mut error = ExplainError::new(
                "WrongContract",
                format!(
                    "the document is {tag}; this lens reads {RUNNER_OUTPUT_CONTRACT} \
                     (kernel contract 2.0) only, and refuses any other tag without \
                     reading the body"
                ),
            );
            error.path = Some("contract".into());
            Err(error)
        }
        _ => Ok(()),
    }
}

fn read_envelope(runner_output: &Value) -> Result<Envelope, ExplainError> {
    check_tag(runner_output)?;
    canonical::check_integers(runner_output)?;
    serde_json::from_value(runner_output.clone())
        .map_err(|error| ExplainError::new("MalformedRunnerOutput", error.to_string()))
}

/// Refuse a package that holds a belief of a higher order than the
/// caller's depth policy allows (`depth.max_order` of an
/// `enaction-policies/1` document), with the validator's rule name
/// `nesting-bounded` and the path `package.beliefs[belief_id=N].holders`
/// of the first such belief in document order.
///
/// The validator applies this rule in a preview; the lens repeats it so
/// that a reader of a vendored corpus can refuse what the corpus refuses
/// without running Enaction code.
pub fn check_depth(runner_output: &Value, policies: &Value) -> Result<(), ExplainError> {
    if policies["contract"] != "enaction-policies/1" {
        return Err(ExplainError::new(
            "MalformedPolicies",
            "the policies document is not enaction-policies/1",
        ));
    }
    let max_order = policies["depth"]["max_order"].as_u64().ok_or_else(|| {
        ExplainError::new(
            "MalformedPolicies",
            "the policies document has no integer depth.max_order",
        )
    })?;
    let envelope = read_envelope(runner_output)?;
    for belief in &envelope.package.beliefs {
        let order = belief.holders.len() as u64;
        if order > max_order {
            let mut error = ExplainError::new(
                "nesting-bounded",
                format!(
                    "belief {} has order {order}, above the declared depth.max_order {max_order}",
                    belief.belief_id
                ),
            );
            error.path = Some(format!(
                "package.beliefs[belief_id={}].holders",
                belief.belief_id
            ));
            return Err(error);
        }
    }
    Ok(())
}

fn finding(rule: &'static str, path: String, detail: String) -> ExplainError {
    let mut error = ExplainError::new(rule, detail);
    error.path = Some(path);
    error
}

/// Refuse a package whose supersession links break a rule of kernel
/// contract 2.0, with every finding, from the contract text alone
/// (CAC-KERNEL section 6; enaction-engine ADR-0029 and ADR-0030):
///
/// * `supersedes-present`: the link names no belief of the package, at
///   `package.beliefs[belief_id=N].supersedes`;
/// * the six cause rules, at `package.beliefs[belief_id=N].supersedes.because`
///   with `N` the successor, in the order of the contract's table, after
///   `supersedes-present` for the same link. `supersession-cause-present`
///   (no event of the package has the id) is then the only finding for that
///   link. Otherwise `-observed` (the event's observer is not the revising
///   mind: the successor's outermost holder, or its bearer when it has
///   none), `-cited` (the event is not among the successor's
///   `source_events`), `-new` (it is among those of the belief replaced),
///   `-not-earlier` (its tick is before the replaced belief's `valid_from`)
///   and `-not-later` (its tick is after the successor's `valid_from`).
///   With no belief to replace, `-new` and `-not-earlier` are skipped;
/// * `belief-key-chained`: the beliefs of one key (`holders`, `bearer`,
///   `proposition`) form one chain under in-key links, at
///   `package.beliefs[belief_id=N].supersedes`. A belief of the key that
///   supersedes no belief of the key, other than the earliest by
///   (`valid_from`, id), is a silent overwrite; a belief that supersedes a
///   belief of the key that another belief of the key, earlier by id,
///   already supersedes is a fork.
///
/// The link findings come first, in document order, then the key findings,
/// key by key. That order is this lens's own, not the validator's.
/// Other package rules (`supersession-acyclic`, the digest, the status
/// policy and the rest of CAC-KERNEL section 6) are the validator's alone.
/// A runner output with another tag gives the one `WrongContract` finding.
pub fn check_links(runner_output: &Value) -> Result<(), Vec<ExplainError>> {
    let envelope = read_envelope(runner_output).map_err(|error| vec![error])?;
    let world = World::read(&envelope);
    let beliefs = &envelope.package.beliefs;
    let by_id: BTreeMap<u64, &Belief> = beliefs.iter().map(|b| (b.belief_id, b)).collect();
    let mut findings = Vec::new();

    for belief in beliefs {
        let Some(link) = belief.supersedes else {
            continue;
        };
        let id = belief.belief_id;
        let path = format!("package.beliefs[belief_id={id}].supersedes");
        let because = format!("{path}.because");
        let replaced = by_id.get(&link.belief).copied();
        if replaced.is_none() {
            findings.push(finding(
                "supersedes-present",
                path.clone(),
                format!(
                    "belief {id} supersedes belief {}, which the package does not hold",
                    link.belief
                ),
            ));
        }
        let Some(event) = world.events.get(&link.because) else {
            findings.push(finding(
                "supersession-cause-present",
                because,
                format!(
                    "belief {id} names event {} as its cause, which the package does not hold",
                    link.because
                ),
            ));
            continue;
        };
        let tick = event.get("tick").and_then(Value::as_u64);
        let observer = event.get("observer").and_then(Value::as_str);
        let revising = belief.holders.first().unwrap_or(&belief.bearer);
        if observer != Some(revising.as_str()) {
            findings.push(finding(
                "supersession-cause-observed",
                because.clone(),
                format!(
                    "event {} was observed by {}, not by {revising}, the mind that revises belief {id}",
                    link.because,
                    observer.unwrap_or("no one")
                ),
            ));
        }
        if !belief.source_events.contains(&link.because) {
            findings.push(finding(
                "supersession-cause-cited",
                because.clone(),
                format!(
                    "belief {id} does not rest on event {}, its cause",
                    link.because
                ),
            ));
        }
        if let Some(replaced) = replaced {
            if replaced.source_events.contains(&link.because) {
                findings.push(finding(
                    "supersession-cause-new",
                    because.clone(),
                    format!(
                        "belief {} already rests on event {}, so it is not new evidence",
                        replaced.belief_id, link.because
                    ),
                ));
            }
            if let Some(tick) = tick
                && tick < replaced.valid_from
            {
                findings.push(finding(
                    "supersession-cause-not-earlier",
                    because.clone(),
                    format!(
                        "event {} is on tick {tick}, before belief {} starts on tick {}",
                        link.because, replaced.belief_id, replaced.valid_from
                    ),
                ));
            }
        }
        if let Some(tick) = tick
            && tick > belief.valid_from
        {
            findings.push(finding(
                "supersession-cause-not-later",
                because,
                format!(
                    "event {} is on tick {tick}, after belief {id} starts on tick {}",
                    link.because, belief.valid_from
                ),
            ));
        }
    }

    type Key<'b> = (&'b [String], &'b str, &'b str);
    let mut keys: BTreeMap<Key, Vec<&Belief>> = BTreeMap::new();
    for belief in beliefs {
        keys.entry((
            belief.holders.as_slice(),
            belief.bearer.as_str(),
            belief.proposition.as_str(),
        ))
        .or_default()
        .push(belief);
    }
    for members in keys.values_mut() {
        members.sort_by_key(|belief| belief.belief_id);
        let earliest = members
            .iter()
            .min_by_key(|belief| (belief.valid_from, belief.belief_id))
            .map(|belief| belief.belief_id);
        let in_key: BTreeSet<u64> = members.iter().map(|belief| belief.belief_id).collect();
        let mut superseded_by: BTreeMap<u64, u64> = BTreeMap::new();
        for belief in members.iter() {
            let id = belief.belief_id;
            let path = format!("package.beliefs[belief_id={id}].supersedes");
            match belief.replaces().filter(|target| in_key.contains(target)) {
                None if Some(id) != earliest => findings.push(finding(
                    "belief-key-chained",
                    path,
                    format!(
                        "belief {id} supersedes no earlier belief of its key: a silent overwrite"
                    ),
                )),
                None => {}
                Some(target) => {
                    if let Some(first) = superseded_by.get(&target) {
                        findings.push(finding(
                            "belief-key-chained",
                            path,
                            format!(
                                "belief {id} supersedes belief {target}, which belief {first} already supersedes: a fork"
                            ),
                        ));
                    } else {
                        superseded_by.insert(target, id);
                    }
                }
            }
        }
    }

    if findings.is_empty() {
        Ok(())
    } else {
        Err(findings)
    }
}

/// Every model of the package (a belief of order 1 or more), in belief-id
/// order, with its timeline, its `until` when it is superseded, and
/// whether it is stale-divergent: the shape of `expected.json` in
/// Enaction's `conformance/tom/v2`. Derived from the package alone, by the
/// rule of CAC-KERNEL section 9, "Models are compared with minds"; world
/// truth is not read.
///
/// A model whose compared belief is not unique at some tick of its
/// lifetime refuses the derivation with `model-counterpart-unique`. The
/// report ([`explain`]) shows such a model as refused instead.
pub fn model_timelines(runner_output: &Value) -> Result<Value, ExplainError> {
    let envelope = read_envelope(runner_output)?;
    let ledger = Ledger::new(&envelope.package.beliefs);
    let mut out = Vec::new();
    for model in ledger.models() {
        let derived = ledger.derive(model);
        let timeline = derived.timeline.map_err(|tick| {
            let mut error = ExplainError::new(
                "model-counterpart-unique",
                format!(
                    "belief {}: more than one compared belief holds at tick {tick}",
                    model.belief_id
                ),
            );
            error.path = Some(format!("package.beliefs[belief_id={}]", model.belief_id));
            error
        })?;
        let mut row = Map::new();
        row.insert("belief_id".into(), json!(model.belief_id));
        row.insert(
            "timeline".into(),
            Value::Array(
                timeline
                    .iter()
                    .map(|(from, verdict)| json!({"from_tick": from, "verdict": verdict.name()}))
                    .collect(),
            ),
        );
        if let Some(until) = derived.until {
            row.insert("until".into(), json!(until));
        }
        row.insert("stale_divergent".into(), json!(stale_divergent(&timeline)));
        out.push(Value::Object(row));
    }
    Ok(json!({"models": out}))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Verdict {
    Agrees,
    Diverges,
    NoClaim,
}

impl Verdict {
    fn name(self) -> &'static str {
        match self {
            Self::Agrees => "agrees",
            Self::Diverges => "diverges",
            Self::NoClaim => "no-claim",
        }
    }
}

/// First segment agrees and a later one diverges (CAC-KERNEL section 9).
fn stale_divergent(timeline: &[(u64, Verdict)]) -> bool {
    matches!(timeline.first(), Some((_, Verdict::Agrees)))
        && timeline
            .iter()
            .skip(1)
            .any(|(_, verdict)| *verdict == Verdict::Diverges)
}

struct Derived {
    until: Option<u64>,
    /// The segments, or the first tick where more than one compared belief
    /// holds.
    timeline: Result<Vec<(u64, Verdict)>, u64>,
}

/// The beliefs of one package with the tick at which each is ended by a
/// belief that names it in `supersedes`.
struct Ledger<'a> {
    beliefs: &'a [Belief],
    ended_at: BTreeMap<u64, u64>,
}

impl<'a> Ledger<'a> {
    fn new(beliefs: &'a [Belief]) -> Self {
        let mut ended_at: BTreeMap<u64, u64> = BTreeMap::new();
        for belief in beliefs {
            if let Some(ended) = belief.replaces() {
                let tick = ended_at.entry(ended).or_insert(belief.valid_from);
                *tick = (*tick).min(belief.valid_from);
            }
        }
        Self { beliefs, ended_at }
    }

    fn models(&self) -> Vec<&'a Belief> {
        let mut models: Vec<&Belief> = self
            .beliefs
            .iter()
            .filter(|belief| !belief.holders.is_empty())
            .collect();
        models.sort_by_key(|belief| belief.belief_id);
        models
    }

    fn until(&self, belief: &Belief) -> Option<u64> {
        self.ended_at.get(&belief.belief_id).copied()
    }

    /// Holding at tick `t`: started by `t` and not ended by `t`.
    fn holds_at(&self, belief: &Belief, tick: u64) -> bool {
        belief.valid_from <= tick && self.until(belief).is_none_or(|end| end > tick)
    }

    /// Beliefs of the compared kind: the outermost holder removed, the same
    /// bearer and proposition.
    fn compared(&self, model: &Belief) -> Vec<&'a Belief> {
        let path = &model.holders[1..];
        let mut out: Vec<&Belief> = self
            .beliefs
            .iter()
            .filter(|belief| {
                belief.holders == path
                    && belief.bearer == model.bearer
                    && belief.proposition == model.proposition
            })
            .collect();
        out.sort_by_key(|belief| belief.belief_id);
        out
    }

    fn derive(&self, model: &Belief) -> Derived {
        let start = model.valid_from;
        let until = self.until(model);
        if model.status == Status::Unknown {
            return Derived {
                until,
                timeline: Ok(vec![(start, Verdict::NoClaim)]),
            };
        }
        // A successor that starts no later than the model leaves one tick.
        let end = until.map(|until| until.max(start + 1));
        let compared = self.compared(model);
        let mut ticks: BTreeSet<u64> = BTreeSet::from([start]);
        for belief in &compared {
            ticks.insert(belief.valid_from);
            if let Some(ended) = self.until(belief) {
                ticks.insert(ended);
            }
        }
        let claim = model.status.holds();
        let mut timeline: Vec<(u64, Verdict)> = Vec::new();
        for tick in ticks
            .into_iter()
            .filter(|tick| *tick >= start && end.is_none_or(|end| *tick < end))
        {
            let holding: Vec<&&Belief> = compared
                .iter()
                .filter(|belief| self.holds_at(belief, tick))
                .collect();
            if holding.len() > 1 {
                return Derived {
                    until,
                    timeline: Err(tick),
                };
            }
            let held = holding.first().is_some_and(|belief| belief.status.holds());
            let verdict = if claim == held {
                Verdict::Agrees
            } else {
                Verdict::Diverges
            };
            if timeline.last().map(|(_, last)| *last) != Some(verdict) {
                timeline.push((tick, verdict));
            }
        }
        Derived {
            until,
            timeline: Ok(timeline),
        }
    }

    /// The report row of one model: who models whom about what, the
    /// model's cited causes, the beliefs it is compared with (with their
    /// own world comparison where they are of order 0), the world's truth
    /// value of the proposition as a separate fact, and the timeline.
    fn model_row(&self, model: &Belief, world: &World) -> Value {
        let derived = self.derive(model);
        let segment_end = |index: usize, timeline: &[(u64, Verdict)]| -> Option<u64> {
            match timeline.get(index + 1) {
                Some((next, _)) => Some(next - 1),
                None => derived
                    .until
                    .map(|until| until.max(model.valid_from + 1) - 1),
            }
        };
        let (timeline, refused, stale) = match &derived.timeline {
            Ok(timeline) => (
                timeline
                    .iter()
                    .enumerate()
                    .map(|(index, (from, verdict))| {
                        json!({
                            "from_tick": from,
                            "last_tick": segment_end(index, timeline),
                            "verdict": verdict.name(),
                        })
                    })
                    .collect::<Vec<_>>(),
                Value::Null,
                stale_divergent(timeline),
            ),
            Err(tick) => (
                Vec::new(),
                json!({"rule": "model-counterpart-unique", "tick": tick}),
                false,
            ),
        };
        let compared: Vec<Value> = self
            .compared(model)
            .into_iter()
            .map(|belief| {
                let in_world_truth = world.truth.contains(belief.proposition.as_str());
                json!({
                    "belief_id": belief.belief_id,
                    "status": belief.status.name(),
                    "holds": belief.status.holds(),
                    "valid_from": belief.valid_from,
                    "until": self.until(belief),
                    "supersedes": belief.replaces(),
                    "supersedes_because": world.link_cause(belief),
                    "world": if belief.holders.is_empty() {
                        json!(belief.status.world(in_world_truth))
                    } else {
                        Value::Null
                    },
                })
            })
            .collect();
        let claim = match model.status {
            Status::Unknown => NO_CLAIM,
            status if status.holds() => "holds",
            _ => "does-not-hold",
        };
        json!({
            "belief_id": model.belief_id,
            "holders": model.holders,
            "bearer": model.bearer,
            "order": model.holders.len(),
            "proposition": model.proposition,
            "status": model.status.name(),
            "claim": claim,
            "confidence": {
                "belief": model.confidence.belief,
                "plausibility": model.confidence.plausibility,
            },
            "valid_from": model.valid_from,
            "until": derived.until,
            "supersedes": model.replaces(),
            "supersedes_because": world.link_cause(model),
            "caused_by": model
                .source_events
                .iter()
                .map(|id| world.cause(*id))
                .collect::<Vec<_>>(),
            "compared_with": {
                "holders": &model.holders[1..],
                "bearer": model.bearer,
                "beliefs": compared,
            },
            "proposition_in_world_truth": world.truth.contains(model.proposition.as_str()),
            "timeline": timeline,
            "stale_divergent": stale,
            "refused": refused,
            "guarantee": guarantee(GuaranteeLevel::Tested, MODEL_EVIDENCE),
        })
    }
}

fn model_summary(models: &[Value]) -> Value {
    let count = |pick: fn(&Value) -> bool| models.iter().filter(|model| pick(model)).count();
    json!({
        "models": models.len(),
        "stale_divergent": count(|model| model["stale_divergent"] == true),
        "refused": count(|model| !model["refused"].is_null()),
    })
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

    /// The event a supersession link names as its cause, with its id, kind,
    /// observer, tick and proposition, or `resolved: false` when the
    /// package holds no such event. `null` when the belief supersedes
    /// nothing.
    fn link_cause(&self, belief: &Belief) -> Value {
        let Some(link) = belief.supersedes else {
            return Value::Null;
        };
        let mut out = Map::new();
        out.insert("id".into(), json!(link.because));
        match self.events.get(&link.because) {
            Some(event) => {
                out.insert("resolved".into(), json!(true));
                for name in ["kind", "observer", "tick", "proposition"] {
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
        // Only order-0 beliefs reach this comparison; models are in `models`.
        let level = guarantee(GuaranteeLevel::Tested, EVIDENCE);
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
            "supersedes": belief.replaces(),
            "supersedes_because": self.link_cause(belief),
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
    let models = &report["model_summary"];
    let _ = writeln!(
        out,
        "models: {}; stale-divergent: {}; refused: {}",
        models["models"], models["stale_divergent"], models["refused"]
    );
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
            render_link(&mut out, "    ", belief);
        }
    }
    for model in report["models"].as_array().into_iter().flatten() {
        let _ = writeln!(out);
        render_model(&mut out, model);
    }
    out
}

/// The line that says which belief a belief replaces and why: "replaces
/// belief N because of event E: kind by observer at tick T, proposition".
/// Nothing for a belief that supersedes nothing.
fn render_link(out: &mut String, indent: &str, belief: &Value) {
    let cause = &belief["supersedes_because"];
    if cause.is_null() {
        return;
    }
    let replaced = text(&belief["supersedes"]);
    if cause["resolved"] == true {
        let _ = writeln!(
            out,
            "{indent}replaces belief {replaced} because of event {}: {} by {} at tick {}, {:?}",
            cause["id"],
            text(&cause["kind"]),
            text(&cause["observer"]),
            text(&cause["tick"]),
            text(&cause["proposition"])
        );
    } else {
        let _ = writeln!(
            out,
            "{indent}replaces belief {replaced} because of event {id} (unresolved: the package has no event {id})",
            id = cause["id"]
        );
    }
}

/// `21..111` for a closed range, `112..` for one that runs to the end of
/// the package.
fn span(first: &Value, last: &Value) -> String {
    if last.is_null() {
        format!("{}..", text(first))
    } else {
        format!("{}..{}", text(first), text(last))
    }
}

fn whose(holders: &Value, bearer: &Value) -> String {
    let holders = words(holders, " > ");
    if holders.is_empty() {
        format!("{}'s own beliefs", text(bearer))
    } else {
        format!("{}'s beliefs as modelled by {holders}", text(bearer))
    }
}

fn render_model(out: &mut String, model: &Value) {
    let claim = match model["claim"].as_str() {
        Some("holds") => "says it holds".to_owned(),
        Some("does-not-hold") => "says it does not hold".to_owned(),
        _ => "makes no claim".to_owned(),
    };
    let _ = writeln!(
        out,
        "model {}: {} models {}: {:?} ({} {}..{}: {}) [{}]",
        text(&model["belief_id"]),
        words(&model["holders"], " > "),
        text(&model["bearer"]),
        text(&model["proposition"]),
        text(&model["status"]),
        model["confidence"]["belief"],
        model["confidence"]["plausibility"],
        claim,
        level(model)
    );
    let mut lifetime = if model["until"].is_null() {
        format!("from tick {} (not superseded)", text(&model["valid_from"]))
    } else {
        format!(
            "from tick {} until tick {}",
            text(&model["valid_from"]),
            text(&model["until"])
        )
    };
    if !model["supersedes"].is_null() {
        let _ = write!(
            lifetime,
            "; supersedes belief {}",
            text(&model["supersedes"])
        );
    }
    let _ = writeln!(out, "  lifetime: {lifetime}");
    render_link(out, "  ", model);
    let causes: Vec<String> = model["caused_by"]
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
    let _ = writeln!(
        out,
        "  caused by: {}",
        if causes.is_empty() {
            "none".to_owned()
        } else {
            causes.join(", ")
        }
    );
    let _ = writeln!(
        out,
        "  world: {} world truth (a separate fact; a model is never compared with it)",
        if model["proposition_in_world_truth"] == true {
            "in"
        } else {
            "absent from"
        }
    );
    let compared = &model["compared_with"];
    let _ = writeln!(
        out,
        "  compared with {} about it:",
        whose(&compared["holders"], &compared["bearer"])
    );
    let beliefs = compared["beliefs"].as_array().cloned().unwrap_or_default();
    if beliefs.is_empty() {
        let _ = writeln!(
            out,
            "    none stated: {} does not hold it at any tick",
            text(&compared["bearer"])
        );
    }
    for belief in &beliefs {
        let last = match belief["until"].as_u64() {
            Some(until) => json!(until.max(belief["valid_from"].as_u64().unwrap_or(0) + 1) - 1),
            None => Value::Null,
        };
        let world = if belief["world"].is_null() {
            String::new()
        } else {
            format!("; {}", text(&belief["world"]))
        };
        let _ = writeln!(
            out,
            "    belief {}: {} ({}) ticks {}{}",
            text(&belief["belief_id"]),
            text(&belief["status"]),
            if belief["holds"] == true {
                "holds"
            } else {
                "does not hold"
            },
            span(&belief["valid_from"], &last),
            world
        );
    }
    if !model["refused"].is_null() {
        let _ = writeln!(
            out,
            "  refused: {} at tick {} (more than one compared belief holds; none is chosen)",
            text(&model["refused"]["rule"]),
            text(&model["refused"]["tick"])
        );
        return;
    }
    for segment in model["timeline"].as_array().into_iter().flatten() {
        let _ = writeln!(
            out,
            "  ticks {}: {}",
            span(&segment["from_tick"], &segment["last_tick"]),
            text(&segment["verdict"])
        );
    }
    if model["stale_divergent"] == true {
        let first_diverging = model["timeline"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|segment| segment["verdict"] == "diverges")
            .map(|segment| text(&segment["from_tick"]))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "  stale-divergent: right when stated, wrong from tick {first_diverging} without being restated [{}]",
            level(model)
        );
    }
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
