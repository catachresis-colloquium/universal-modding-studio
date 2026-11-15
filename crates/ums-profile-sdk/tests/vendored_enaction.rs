// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! Enaction's mind-trace conformance corpus, vendored by digest
//! (CAC-KERNEL section 6: fixtures are the cross-repository currency, and
//! consumers vendor them against a recorded digest).
//!
//! `vendor/enaction/conformance/mind/v1` is a byte copy of
//! `conformance/mind/v1` from enaction-engine at the commit recorded in
//! `vendor/enaction/PROVENANCE.json`.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use support::{check_vendored, copy_tree, read_json, repo_root};
use ums_profile_sdk::explain::{explain, render_text};

fn corpus() -> PathBuf {
    repo_root().join("vendor/enaction/conformance/mind/v1")
}

fn provenance() -> PathBuf {
    repo_root().join("vendor/enaction/PROVENANCE.json")
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
    assert_eq!(record["path"], "conformance/mind/v1");
    let commit = record["commit"].as_str().expect("commit");
    assert_eq!(commit.len(), 40, "{commit}");
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
}

fn scratch_copy(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("vendored-enaction")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    copy_tree(&corpus(), &dir);
    dir
}

#[test]
fn a_changed_vendored_file_fails_the_check() {
    let dir = scratch_copy("changed");
    let path = dir.join("accepted/the-grudge/expected.json");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b' ');
    fs::write(&path, bytes).unwrap();
    let error = check_vendored(&dir, &provenance()).expect_err("changed file");
    assert!(
        error.contains("accepted/the-grudge/expected.json"),
        "{error}"
    );
}

#[test]
fn an_extra_vendored_file_fails_the_check() {
    let dir = scratch_copy("extra");
    fs::write(dir.join("accepted/the-grudge/notes.txt"), "x").unwrap();
    let error = check_vendored(&dir, &provenance()).expect_err("extra file");
    assert!(
        error.contains("+ ") && error.contains("notes.txt"),
        "{error}"
    );
}

#[test]
fn a_missing_vendored_file_fails_the_check() {
    let dir = scratch_copy("missing");
    fs::remove_file(dir.join("rejected/BadRanking/mind-trace.json")).unwrap();
    let error = check_vendored(&dir, &provenance()).expect_err("missing file");
    assert!(
        error.contains("- ") && error.contains("BadRanking"),
        "{error}"
    );
}

#[test]
fn a_file_changed_together_with_its_sum_fails_on_the_pinned_digest() {
    let dir = scratch_copy("resummed");
    let path = dir.join("policies.json");
    fs::write(&path, b"{}\n").unwrap();
    let sums = support::regenerate_sums(&dir);
    fs::write(dir.join("SHA256SUMS"), sums).unwrap();
    let error = check_vendored(&dir, &provenance()).expect_err("resummed");
    assert!(error.contains("pinned"), "{error}");
}

fn accepted(case: &str) -> Value {
    let dir = corpus();
    explain(
        &read_json(&dir.join("runner-output.json")),
        Some(&read_json(
            &dir.join("accepted").join(case).join("mind-trace.json"),
        )),
        false,
    )
    .unwrap_or_else(|error| panic!("{case}: {error}"))
}

/// Commitment-override (agent, tick) pairs from a report, sorted.
fn overrides(report: &Value) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = report["agents"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|agent| {
            let id = agent["agent"].as_str().unwrap().to_owned();
            agent["commitment_override_at"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |tick| (id.clone(), tick.as_u64().unwrap()))
        })
        .collect();
    out.sort();
    out
}

fn expected(case: &str) -> Vec<(String, u64)> {
    let record = read_json(&corpus().join("accepted").join(case).join("expected.json"));
    let mut out: Vec<(String, u64)> = record["commitment_override"]
        .as_array()
        .expect("commitment_override")
        .iter()
        .map(|item| {
            (
                item["agent"].as_str().unwrap().to_owned(),
                item["tick"].as_u64().unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn the_grudge_reports_exactly_the_expected_commitment_override_ticks() {
    let report = accepted("the-grudge");
    assert_eq!(overrides(&report), expected("the-grudge"));
    // The expected file is not empty, so this comparison can fail.
    assert_eq!(
        expected("the-grudge"),
        vec![("guard-0".to_owned(), 42), ("guard-0".to_owned(), 43)]
    );
    let text = render_text(&report);
    assert!(
        text.contains("agent guard-0: commitment-override ticks: 2 of"),
        "{text}"
    );
}

#[test]
fn the_flinch_reports_no_commitment_override_and_a_startle() {
    let report = accepted("the-flinch");
    assert_eq!(overrides(&report), expected("the-flinch"));
    assert!(expected("the-flinch").is_empty());
    let guard = report["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["agent"] == "guard-0")
        .expect("guard-0");
    assert!(
        guard["tells_at"]["startle"]
            .as_array()
            .is_some_and(|ticks| !ticks.is_empty()),
        "{}",
        guard["tells_at"]
    );
}

#[test]
fn the_vendored_runner_output_belief_agrees_with_its_world() {
    // "contact-3 is hostile" is believed and is the one world-truth atom.
    let report = accepted("the-grudge");
    assert_eq!(
        report["belief_summary"],
        json!({"beliefs": 1, "agrees-with-world": 1, "diverges-from-world": 0, "no-claim": 0})
    );
}
