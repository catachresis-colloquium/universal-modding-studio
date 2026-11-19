// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! Enaction's theory-of-mind conformance corpora, vendored by digest.
//!
//! `vendor/enaction/conformance/tom/v2` is a byte copy of
//! `conformance/tom/v2` from enaction-engine at the commit recorded in
//! `vendor/enaction/conformance/PROVENANCE.json`: the kernel contract 2.0
//! form (ADR-0030, a draft), whose envelopes are
//! `enaction-runner-output/2` and whose supersession links name their
//! cause. The corpus pins the rule of CAC-KERNEL section 9, "Models are
//! compared with minds" (ADR-0028): a model is compared with the modelled
//! bearer's own belief of one order less, over the model's lifetime, and
//! never with world truth.
//!
//! For each accepted case the lens derives, from the runner output alone,
//! exactly the case's `expected.json`. Each rejected case is "refused with
//! exactly that rule" (the corpus README). The lens repeats those rules
//! (`check_depth` for `nesting-bounded`, `check_links` for
//! `belief-key-chained` and the six `supersession-cause-*` rules) so that a
//! reader of the corpus refuses what it refuses without running Enaction
//! code, and this file checks that each rejected case yields exactly one
//! finding, with that rule, at the named path.
//!
//! `vendor/enaction/conformance/tom/v1` is the 1.x form. Enaction froze it,
//! and so does this tree: its sums must still hold, and the lens refuses
//! every package in it for its tag alone, with one finding.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use support::{check_vendored, check_vendored_against, copy_tree, read_json, repo_root};
use ums_profile_sdk::canonical::to_canonical_string;
use ums_profile_sdk::explain::{
    ExplainError, check_depth, check_links, explain, model_timelines, render_text,
};

fn v1() -> PathBuf {
    repo_root().join("vendor/enaction/conformance/tom/v1")
}

fn v1_provenance() -> PathBuf {
    repo_root().join("vendor/enaction/conformance/tom/PROVENANCE.json")
}

fn corpus() -> PathBuf {
    repo_root().join("vendor/enaction/conformance/tom/v2")
}

fn provenance() -> Value {
    read_json(&repo_root().join("vendor/enaction/conformance/PROVENANCE.json"))
}

/// The pinned digest of `tom/v2/SHA256SUMS` in the v2 provenance record.
fn pinned() -> String {
    provenance()["corpora"]
        .as_array()
        .expect("corpora")
        .iter()
        .find(|entry| entry["path"] == "conformance/tom/v2")
        .and_then(|entry| entry["sha256sums_digest"].as_str())
        .expect("a pinned digest for conformance/tom/v2")
        .to_owned()
}

// ---- the frozen v1 corpus ----

#[test]
fn the_frozen_v1_corpus_still_matches_its_sums_and_pinned_digest() {
    check_vendored(&v1(), &v1_provenance()).unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn the_v1_provenance_names_the_upstream_commit_and_path() {
    let record = read_json(&v1_provenance());
    assert_eq!(
        record["repository"],
        "https://github.com/catachresis-colloquium/enaction-engine"
    );
    assert_eq!(record["path"], "conformance/tom/v1");
    let commit = record["commit"].as_str().expect("commit");
    assert_eq!(commit.len(), 40, "{commit}");
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
}

/// Every `runner-output.json` under `dir`, sorted by path.
fn runner_outputs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .is_some_and(|name| name == "runner-output.json")
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// The one finding the lens gives for an envelope of another contract.
fn assert_refused_for_its_tag(error: &ExplainError, found: &str, what: &str) {
    assert_eq!(error.rule(), "WrongContract", "{what}: {error}");
    assert_eq!(error.path(), Some("contract"), "{what}: {error}");
    assert_eq!(
        error.to_string(),
        format!(
            "WrongContract: the document is {found}; this lens reads \
             enaction-runner-output/2 (kernel contract 2.0) only, and refuses \
             any other tag without reading the body"
        ),
        "{what}"
    );
}

#[test]
fn every_v1_package_is_refused_for_its_tag_alone_with_one_finding() {
    let policies = read_json(&v1().join("policies.json"));
    let mut refused = 0;
    for path in runner_outputs(&v1()) {
        let what = path.strip_prefix(v1()).unwrap().display().to_string();
        let document = read_json(&path);
        assert_eq!(document["contract"], "enaction-runner-output/1", "{what}");
        let found = "enaction-runner-output/1";
        assert_refused_for_its_tag(
            &explain(&document, None, false).expect_err("explain"),
            found,
            &what,
        );
        assert_refused_for_its_tag(
            &model_timelines(&document).expect_err("model_timelines"),
            found,
            &what,
        );
        assert_refused_for_its_tag(
            &check_depth(&document, &policies).expect_err("check_depth"),
            found,
            &what,
        );
        let findings = check_links(&document).expect_err("check_links");
        assert_eq!(findings.len(), 1, "{what}: {findings:?}");
        assert_refused_for_its_tag(&findings[0], found, &what);
        refused += 1;
    }
    // Seven accepted cases and one rejected case.
    assert_eq!(refused, 8);
}

// ---- the v2 corpus ----

#[test]
fn the_vendored_corpus_matches_its_sums_and_pinned_digest() {
    check_vendored_against(&corpus(), &pinned()).unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn the_provenance_names_the_upstream_commit_and_both_corpora() {
    let record = provenance();
    assert_eq!(
        record["repository"],
        "https://github.com/catachresis-colloquium/enaction-engine"
    );
    let commit = record["commit"].as_str().expect("commit");
    assert_eq!(commit.len(), 40, "{commit}");
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
    let paths: Vec<&str> = record["corpora"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["conformance/tom/v2", "conformance/mind/v2"]);
}

#[test]
fn a_changed_vendored_file_fails_the_check() {
    for (name, dir, check) in [
        (
            "vendored-tom-v1",
            v1(),
            Box::new(|dir: &Path| check_vendored(dir, &v1_provenance()))
                as Box<dyn Fn(&Path) -> Result<(), String>>,
        ),
        (
            "vendored-tom-v2",
            corpus(),
            Box::new(|dir: &Path| check_vendored_against(dir, &pinned())),
        ),
    ] {
        let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        let _ = fs::remove_dir_all(&scratch);
        copy_tree(&dir, &scratch);
        let path = scratch.join("accepted/false-exposure/expected.json");
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b' ');
        fs::write(&path, bytes).unwrap();
        let error = check(&scratch).expect_err("changed file");
        assert!(
            error.contains("accepted/false-exposure/expected.json"),
            "{name}: {error}"
        );
    }
}

/// The accepted cases, as listed in the corpus README and CAC-KERNEL
/// section 9. The directory listing must hold exactly these.
const ACCEPTED: [&str; 7] = [
    "caught",
    "false-concealment",
    "false-exposure",
    "false-exposure-restated",
    "no-claim",
    "second-order",
    "true-concealment",
];

/// The rejected cases with the path of their one finding: the README names
/// the first two; CAC-KERNEL section 6 gives the path of the cause rules,
/// `beliefs[belief_id=N].supersedes.because` with `N` the successor.
const REJECTED: [(&str, &str); 8] = [
    (
        "belief-key-chained",
        "package.beliefs[belief_id=2].supersedes",
    ),
    ("nesting-bounded", "package.beliefs[belief_id=1].holders"),
    (
        "supersession-cause-cited",
        "package.beliefs[belief_id=2].supersedes.because",
    ),
    (
        "supersession-cause-new",
        "package.beliefs[belief_id=2].supersedes.because",
    ),
    (
        "supersession-cause-not-earlier",
        "package.beliefs[belief_id=2].supersedes.because",
    ),
    (
        "supersession-cause-not-later",
        "package.beliefs[belief_id=2].supersedes.because",
    ),
    (
        "supersession-cause-observed",
        "package.beliefs[belief_id=4].supersedes.because",
    ),
    (
        "supersession-cause-present",
        "package.beliefs[belief_id=2].supersedes.because",
    ),
];

fn case_dir(kind: &str, case: &str) -> PathBuf {
    corpus().join(kind).join(case)
}

fn listing(kind: &str) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(corpus().join(kind))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    found.sort();
    found
}

/// The policies a case is read under: its own when it has one, else the
/// corpus file.
fn policies_for(case: &str) -> Value {
    let own = case_dir("accepted", case).join("policies.json");
    if own.is_file() {
        read_json(&own)
    } else {
        read_json(&corpus().join("policies.json"))
    }
}

#[test]
fn the_accepted_cases_are_exactly_the_seven_listed() {
    assert_eq!(listing("accepted"), ACCEPTED);
}

#[test]
fn every_accepted_case_derives_exactly_its_expected_json() {
    let mut checked = 0;
    for case in ACCEPTED {
        let dir = case_dir("accepted", case);
        let runner_output = read_json(&dir.join("runner-output.json"));
        assert_eq!(
            runner_output["contract"], "enaction-runner-output/2",
            "{case}"
        );
        check_depth(&runner_output, &policies_for(case))
            .unwrap_or_else(|error| panic!("{case}: refused under its own policies: {error}"));
        check_links(&runner_output)
            .unwrap_or_else(|findings| panic!("{case}: link findings {findings:?}"));
        let derived =
            model_timelines(&runner_output).unwrap_or_else(|error| panic!("{case}: {error}"));
        let expected = read_json(&dir.join("expected.json"));
        assert_eq!(derived, expected, "{case}");
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn the_rejected_cases_are_exactly_the_eight_listed() {
    let names: Vec<&str> = REJECTED.iter().map(|(name, _)| *name).collect();
    assert_eq!(listing("rejected"), names);
}

#[test]
fn each_rejected_case_is_refused_with_exactly_its_rule_at_its_path() {
    let corpus_policies = read_json(&corpus().join("policies.json"));
    let mut refused = 0;
    for (case, path) in REJECTED {
        let runner_output = read_json(&case_dir("rejected", case).join("runner-output.json"));
        let mut findings: Vec<ExplainError> = Vec::new();
        if let Err(error) = check_depth(&runner_output, &corpus_policies) {
            findings.push(error);
        }
        if let Err(errors) = check_links(&runner_output) {
            findings.extend(errors);
        }
        assert_eq!(findings.len(), 1, "{case}: {findings:?}");
        assert_eq!(findings[0].rule(), case, "{case}: {}", findings[0]);
        assert_eq!(findings[0].path(), Some(path), "{case}: {}", findings[0]);
        refused += 1;
    }
    assert_eq!(refused, 8);
}

#[test]
fn second_order_is_refused_under_the_corpus_depth_and_accepted_under_its_own() {
    // The corpus README: "Under the corpus file, this case is refused as
    // nesting-bounded."
    let runner_output = read_json(&case_dir("accepted", "second-order").join("runner-output.json"));
    let error = check_depth(&runner_output, &read_json(&corpus().join("policies.json")))
        .expect_err("order 2 under depth 1");
    assert_eq!(error.rule(), "nesting-bounded", "{error}");
    assert_eq!(
        error.path(),
        Some("package.beliefs[belief_id=4].holders"),
        "{error}"
    );
    check_depth(&runner_output, &policies_for("second-order")).expect("depth 2");
}

#[test]
fn a_package_whose_policies_have_no_depth_is_refused_not_waved_through() {
    let runner_output = read_json(&case_dir("accepted", "caught").join("runner-output.json"));
    let error = check_depth(&runner_output, &json!({"contract": "enaction-policies/1"}))
        .expect_err("no depth policy");
    assert_eq!(error.rule(), "MalformedPolicies", "{error}");
}

fn caught() -> Value {
    read_json(&case_dir("accepted", "caught").join("runner-output.json"))
}

fn own_belief(id: u64, valid_from: u64, status: &str) -> Value {
    json!({
        "belief_id": id,
        "bearer": "contact-21",
        "proposition": "guard-0 at cell-14",
        "status": status,
        "confidence": {"belief": 8000, "plausibility": 9500},
        "source_events": [1],
        "valid_from": valid_from,
    })
}

#[test]
fn two_compared_beliefs_holding_inside_a_lifetime_refuse_the_model() {
    // caught: model 2 lives from tick 2 until tick 5. A second own belief of
    // contact-21 from tick 3, superseding nothing, holds beside belief 1.
    // check_links refuses that package as belief-key-chained; the reader
    // keeps its own refusal for a package handed to it directly.
    let mut document = caught();
    document["package"]["beliefs"]
        .as_array_mut()
        .unwrap()
        .push(own_belief(9, 3, "believed"));
    let findings = check_links(&document).expect_err("an unlinked belief of one key");
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].rule(), "belief-key-chained");
    assert_eq!(
        findings[0].path(),
        Some("package.beliefs[belief_id=9].supersedes")
    );
    let error = model_timelines(&document).expect_err("undecidable");
    assert_eq!(error.rule(), "model-counterpart-unique", "{error}");
    assert!(error.to_string().contains("belief 2"), "{error}");
    assert!(error.to_string().contains("tick 3"), "{error}");

    // The report shows the refusal on that model and does not choose.
    let report = explain(&document, None, false).expect("the report still renders");
    let model = report["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["belief_id"] == 2)
        .expect("model 2")
        .clone();
    assert_eq!(model["refused"]["rule"], "model-counterpart-unique");
    assert_eq!(model["refused"]["tick"], 3);
    assert_eq!(model["timeline"], json!([]));
    let text = render_text(&report);
    assert!(
        text.contains("refused: model-counterpart-unique at tick 3"),
        "{text}"
    );
}

#[test]
fn an_ambiguity_outside_the_lifetime_does_not_refuse_the_model() {
    // Two own beliefs from tick 0; another agent's belief on tick 1 names the
    // second in `supersedes`, which ends it (CAC-KERNEL section 9: whatever
    // its bearer). From tick 1 only one holds, so true-concealment's model
    // (from tick 2) is decidable, and belief 7 is not held: it agrees.
    let mut document =
        read_json(&case_dir("accepted", "true-concealment").join("runner-output.json"));
    let beliefs = document["package"]["beliefs"].as_array_mut().unwrap();
    beliefs.push(own_belief(7, 0, "rejected"));
    beliefs.push(own_belief(8, 0, "believed"));
    let mut ender = own_belief(9, 1, "believed");
    ender["bearer"] = json!("someone-else");
    ender["supersedes"] = json!({"belief": 8, "because": 1});
    beliefs.push(ender);
    let derived = model_timelines(&document).expect("decidable");
    assert_eq!(
        derived["models"][0],
        json!({"belief_id": 1, "timeline": [{"from_tick": 2, "verdict": "agrees"}], "stale_divergent": false})
    );
}

#[test]
fn a_model_is_never_compared_with_world_truth() {
    // Emptying world truth, or filling it, changes no model verdict.
    for case in ACCEPTED {
        let dir = case_dir("accepted", case);
        let original = read_json(&dir.join("runner-output.json"));
        let expected = read_json(&dir.join("expected.json"));
        for truth in [
            json!([]),
            json!(["guard-0 at cell-14", "contact-21 sees guard-0"]),
        ] {
            let mut document = original.clone();
            document["world_truth"] = truth;
            assert_eq!(model_timelines(&document).unwrap(), expected, "{case}");
        }
    }
}

// ---- the cause of a supersession, in the report ----

fn golden_path(extension: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/explain")
        .join(format!(
            "tom-v2-false-exposure-restated.report.golden.{extension}"
        ))
}

fn restated_report() -> Value {
    let document =
        read_json(&case_dir("accepted", "false-exposure-restated").join("runner-output.json"));
    explain(&document, None, false).expect("report")
}

#[test]
fn the_false_exposure_restated_report_matches_its_golden_files() {
    // No bless mode: a changed report is regenerated by hand and reviewed.
    let report = restated_report();
    assert_eq!(
        format!("{}\n", to_canonical_string(&report).unwrap()),
        fs::read_to_string(golden_path("json")).unwrap()
    );
    assert_eq!(
        render_text(&report),
        fs::read_to_string(golden_path("txt")).unwrap()
    );
}

#[test]
fn each_supersession_shows_the_belief_it_replaces_and_its_cause() {
    // false-exposure-restated: contact-21's belief 3 replaces belief 1
    // because of event 3, which contact-21 observed on tick 6; guard-0's
    // model 4 replaces model 2 because of event 4, guard-0's inference on
    // tick 6.
    let report = restated_report();
    let own = report["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["agent"] == "contact-21")
        .expect("contact-21")["beliefs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|belief| belief["belief_id"] == 3)
        .expect("belief 3")
        .clone();
    assert_eq!(own["supersedes"], 1);
    assert_eq!(
        own["supersedes_because"],
        json!({"id": 3, "resolved": true, "kind": "observation", "observer": "contact-21", "tick": 6, "proposition": "guard-0 at cell-14"})
    );
    let model = report["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["belief_id"] == 4)
        .expect("model 4")
        .clone();
    assert_eq!(model["supersedes"], 2);
    assert_eq!(
        model["supersedes_because"],
        json!({"id": 4, "resolved": true, "kind": "inference", "observer": "guard-0", "tick": 6, "proposition": "contact-21 sees guard-0"})
    );
    // A belief that replaces nothing has no cause.
    let first = report["models"][0].clone();
    assert_eq!(first["belief_id"], 2);
    assert_eq!(first["supersedes"], Value::Null);
    assert_eq!(first["supersedes_because"], Value::Null);

    let text = render_text(&report);
    for line in [
        "  belief 3: \"guard-0 at cell-14\" (unknown 0..10000); in world truth -> no-claim [tested]\n    replaces belief 1 because of event 3: observation by contact-21 at tick 6, \"guard-0 at cell-14\"\n",
        "  lifetime: from tick 6 (not superseded); supersedes belief 2\n  replaces belief 2 because of event 4: inference by guard-0 at tick 6, \"contact-21 sees guard-0\"\n",
    ] {
        assert!(text.contains(line), "missing {line:?}\n{text}");
    }
    assert_eq!(text.matches("replaces belief").count(), 2, "{text}");
}

#[test]
fn an_unresolved_cause_is_shown_not_dropped() {
    // The supersession-cause-present case names event 9, which the package
    // lacks. The lens shows that, and refuses nothing for it.
    let document =
        read_json(&case_dir("rejected", "supersession-cause-present").join("runner-output.json"));
    let report = explain(&document, None, false).expect("report");
    let belief = report["agents"][0]["beliefs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|belief| belief["belief_id"] == 2)
        .expect("belief 2")
        .clone();
    assert_eq!(
        belief["supersedes_because"],
        json!({"id": 9, "resolved": false})
    );
    let text = render_text(&report);
    assert!(
        text.contains(
            "    replaces belief 1 because of event 9 (unresolved: the package has no event 9)\n"
        ),
        "{text}"
    );
}

#[test]
fn the_timing_rules_bite_one_tick_past_each_bound() {
    // caught: belief 3 (contact-21, from tick 5) replaces belief 1 (from
    // tick 1) because of event 3 (tick 5). CAC-KERNEL section 6 allows
    // `1 <= cause tick <= 5`. The rejected cases of the corpus sit two or
    // more ticks outside; this pins each bound to the tick.
    let with_cause_tick = |tick: u64| {
        let mut document = caught();
        document["package"]["events"][2]["tick"] = json!(tick);
        check_links(&document)
    };
    let rules = |result: Result<(), Vec<ExplainError>>| -> Vec<&'static str> {
        result
            .err()
            .unwrap_or_default()
            .iter()
            .map(ExplainError::rule)
            .collect()
    };
    assert_eq!(rules(with_cause_tick(5)), Vec::<&str>::new());
    assert_eq!(rules(with_cause_tick(6)), ["supersession-cause-not-later"]);
    assert_eq!(rules(with_cause_tick(1)), Vec::<&str>::new());
    // Belief 1 starts on tick 1; move it to tick 2 so that a cause on tick
    // 1 is one tick early.
    let mut document = caught();
    document["package"]["beliefs"][0]["valid_from"] = json!(2);
    document["package"]["events"][2]["tick"] = json!(1);
    assert_eq!(
        rules(check_links(&document)),
        ["supersession-cause-not-earlier"]
    );
}

#[test]
fn the_adapter_page_cause_excerpt_is_a_block_of_the_golden() {
    // docs/ENACTION-ADAPTER.adoc ("Explain lens") quotes the cause line of
    // false-exposure-restated. The quote must appear in the golden verbatim.
    let page = fs::read_to_string(repo_root().join("docs/ENACTION-ADAPTER.adoc")).unwrap();
    let block = page
        .split("case `false-exposure-restated`:\n\n[source,text]\n----\n")
        .nth(1)
        .and_then(|rest| rest.split("\n----\n").next())
        .expect("the cause excerpt");
    assert!(
        block.contains("replaces belief 1 because of event 3"),
        "{block}"
    );
    let golden = fs::read_to_string(golden_path("txt")).unwrap();
    assert!(
        golden.contains(&format!("\n{block}\n")),
        "the page's excerpt is not in the golden:\n{block}"
    );
}
