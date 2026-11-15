// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! Enaction's theory-of-mind conformance corpus, vendored by digest.
//!
//! `vendor/enaction/conformance/tom/v1` is a byte copy of
//! `conformance/tom/v1` from enaction-engine at the commit recorded in
//! `vendor/enaction/conformance/tom/PROVENANCE.json`. The corpus pins the
//! rule of CAC-KERNEL section 9, "Models are compared with minds"
//! (ADR-0028): a model is compared with the modelled bearer's own belief of
//! one order less, over the model's lifetime, and never with world truth.
//!
//! For each accepted case the lens derives, from the runner output alone,
//! exactly the case's `expected.json`. For the rejected case it does what
//! the corpus README says a reader must: refuse the package under the
//! corpus policies with the existing `nesting-bounded` rule at
//! `package.beliefs[belief_id=1].holders`, and derive nothing.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use support::{check_vendored, copy_tree, read_json, repo_root};
use ums_profile_sdk::explain::{check_depth, explain, model_timelines, render_text};

fn corpus() -> PathBuf {
    repo_root().join("vendor/enaction/conformance/tom/v1")
}

fn provenance() -> PathBuf {
    repo_root().join("vendor/enaction/conformance/tom/PROVENANCE.json")
}

#[test]
fn the_vendored_corpus_matches_its_sums_and_pinned_digest() {
    check_vendored(&corpus(), &provenance()).unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn the_provenance_names_the_upstream_commit_and_path() {
    let record = read_json(&provenance());
    assert_eq!(
        record["repository"],
        "https://github.com/catachresis-colloquium/enaction-engine"
    );
    assert_eq!(record["path"], "conformance/tom/v1");
    let commit = record["commit"].as_str().expect("commit");
    assert_eq!(commit.len(), 40, "{commit}");
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
}

#[test]
fn a_changed_vendored_file_fails_the_check() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("vendored-tom");
    let _ = fs::remove_dir_all(&dir);
    copy_tree(&corpus(), &dir);
    let path = dir.join("accepted/false-exposure/expected.json");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b' ');
    fs::write(&path, bytes).unwrap();
    let error = check_vendored(&dir, &provenance()).expect_err("changed file");
    assert!(
        error.contains("accepted/false-exposure/expected.json"),
        "{error}"
    );
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

fn case_dir(kind: &str, case: &str) -> PathBuf {
    corpus().join(kind).join(case)
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
    let mut found: Vec<String> = fs::read_dir(corpus().join("accepted"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    found.sort();
    assert_eq!(found, ACCEPTED);
}

#[test]
fn every_accepted_case_derives_exactly_its_expected_json() {
    let mut checked = 0;
    for case in ACCEPTED {
        let dir = case_dir("accepted", case);
        let runner_output = read_json(&dir.join("runner-output.json"));
        check_depth(&runner_output, &policies_for(case))
            .unwrap_or_else(|error| panic!("{case}: refused under its own policies: {error}"));
        let derived =
            model_timelines(&runner_output).unwrap_or_else(|error| panic!("{case}: {error}"));
        let expected = read_json(&dir.join("expected.json"));
        assert_eq!(derived, expected, "{case}");
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn the_rejected_case_is_refused_as_nesting_bounded_at_the_named_path() {
    let runner_output =
        read_json(&case_dir("rejected", "nesting-bounded").join("runner-output.json"));
    let error = check_depth(&runner_output, &read_json(&corpus().join("policies.json")))
        .expect_err("an order-2 package under depth 1 is refused");
    assert_eq!(error.rule(), "nesting-bounded", "{error}");
    assert_eq!(
        error.path(),
        Some("package.beliefs[belief_id=1].holders"),
        "{error}"
    );
    let rejected: Vec<String> = fs::read_dir(corpus().join("rejected"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(rejected, ["nesting-bounded"]);
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
    let mut document = caught();
    document["package"]["beliefs"]
        .as_array_mut()
        .unwrap()
        .push(own_belief(9, 3, "believed"));
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
    ender["supersedes"] = json!(8);
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
