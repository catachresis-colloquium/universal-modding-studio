//! Documents written by the real IDApTIK corridor runner
//! (`idaptik-weevil-replay`, Evil Weevil kernel, ABI 1.1), vendored by
//! digest from IDApTIK `crates/idaptik-weevil/tests/fixtures/enaction` at
//! the commit in `vendor/idaptik/PROVENANCE.json`. IDApTIK's README there
//! says every pair exits 0 from `enaction-validate`; that is IDApTIK's
//! claim and is not re-checked here (no validator runs in this test).
//!
//! The explain lens runs over each scenario and its report is pinned as a
//! golden file in `tests/fixtures/weevil-corridor`. The figures asserted
//! below were measured by the coordinator on the real chain on 2026-11-14:
//! decoy-lure 36 beliefs, every one diverging (no truth atoms);
//! cautious-sentry 36 beliefs, every one agreeing; the-grudge commitment
//! override on 37 ticks, 20 to 56; the-grudge-control none; the-flinch one
//! startle on tick 10; the-flinch-control none.

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use support::{check_vendored, copy_tree, read_json, repo_root, sha256_hex};
use ums_profile_sdk::canonical::to_canonical_string;
use ums_profile_sdk::explain::{explain, render_text};

const SCENARIOS: [&str; 6] = [
    "decoy-lure",
    "cautious-sentry",
    "the-grudge",
    "the-grudge-control",
    "the-flinch",
    "the-flinch-control",
];

fn corridor() -> PathBuf {
    repo_root().join("vendor/idaptik/weevil-corridor")
}

fn provenance() -> PathBuf {
    repo_root().join("vendor/idaptik/PROVENANCE.json")
}

fn report(scenario: &str) -> Value {
    let dir = corridor();
    let runner_output = read_json(&dir.join(format!("{scenario}.runner-output.json")));
    let trace_path = dir.join(format!("{scenario}.mind-trace.json"));
    let mind_trace = trace_path.exists().then(|| read_json(&trace_path));
    explain(&runner_output, mind_trace.as_ref(), false)
        .unwrap_or_else(|error| panic!("{scenario}: {error}"))
}

fn guard(report: &Value) -> &Value {
    report["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["agent"] == "guard-0")
        .expect("guard-0")
}

#[test]
fn the_vendored_documents_match_their_sums_and_pinned_digest() {
    check_vendored(&corridor(), &provenance()).unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn the_vendored_policies_match_their_pinned_digest() {
    let record = read_json(&provenance());
    let bytes = fs::read(repo_root().join("vendor/idaptik/enaction-policies.json")).unwrap();
    assert_eq!(
        json!(sha256_hex(&bytes)),
        record["policies"]["sha256"],
        "vendor/idaptik/enaction-policies.json"
    );
}

#[test]
fn every_scenario_has_its_runner_output_and_the_mind_scenarios_a_trace() {
    for scenario in SCENARIOS {
        let dir = corridor();
        assert!(
            dir.join(format!("{scenario}.runner-output.json")).is_file(),
            "{scenario}"
        );
        let has_trace = dir.join(format!("{scenario}.mind-trace.json")).is_file();
        assert_eq!(has_trace, scenario.starts_with("the-"), "{scenario}");
    }
}

#[test]
fn a_changed_or_extra_vendored_document_fails_the_check() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("vendored-idaptik");
    let _ = fs::remove_dir_all(&dir);
    copy_tree(&corridor(), &dir);
    let path = dir.join("the-grudge.mind-trace.json");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b'\n');
    fs::write(&path, bytes).unwrap();
    let error = check_vendored(&dir, &provenance()).expect_err("changed");
    assert!(error.contains("the-grudge.mind-trace.json"), "{error}");

    let _ = fs::remove_dir_all(&dir);
    copy_tree(&corridor(), &dir);
    fs::write(dir.join("extra.json"), "{}").unwrap();
    let error = check_vendored(&dir, &provenance()).expect_err("extra");
    assert!(
        error.contains("+ ") && error.contains("extra.json"),
        "{error}"
    );
}

#[test]
fn decoy_lure_has_36_beliefs_and_every_one_diverges() {
    let report = report("decoy-lure");
    assert_eq!(
        report["belief_summary"],
        json!({"beliefs": 36, "agrees-with-world": 0, "diverges-from-world": 36, "no-claim": 0})
    );
}

#[test]
fn cautious_sentry_has_36_beliefs_and_every_one_agrees() {
    let report = report("cautious-sentry");
    assert_eq!(
        report["belief_summary"],
        json!({"beliefs": 36, "agrees-with-world": 36, "diverges-from-world": 0, "no-claim": 0})
    );
}

#[test]
fn the_grudge_overrides_on_37_ticks_from_20_to_56() {
    let report = report("the-grudge");
    let expected: Vec<u64> = (20..=56).collect();
    assert_eq!(guard(&report)["commitment_override_at"], json!(expected));
    assert_eq!(guard(&report)["commitment_override_ticks"], 37);
}

#[test]
fn the_grudge_control_has_no_commitment_override() {
    let report = report("the-grudge-control");
    for agent in report["agents"].as_array().unwrap() {
        assert_eq!(agent["commitment_override_ticks"], 0, "{}", agent["agent"]);
    }
}

#[test]
fn the_flinch_has_one_startle_on_tick_10() {
    let report = report("the-flinch");
    assert_eq!(guard(&report)["tells_at"]["startle"], json!([10]));
}

#[test]
fn the_flinch_control_has_no_startle() {
    let report = report("the-flinch-control");
    for agent in report["agents"].as_array().unwrap() {
        assert!(
            agent["tells_at"].get("startle").is_none(),
            "{}",
            agent["agent"]
        );
    }
}

fn golden_path(scenario: &str, extension: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/weevil-corridor")
        .join(format!("{scenario}.report.golden.{extension}"))
}

#[test]
fn each_scenario_report_matches_its_golden_files() {
    // No bless mode: a changed report is regenerated by hand and reviewed.
    for scenario in SCENARIOS {
        let report = report(scenario);
        let json_golden = fs::read_to_string(golden_path(scenario, "json"))
            .unwrap_or_else(|error| panic!("{scenario}: {error}"));
        assert_eq!(
            format!("{}\n", to_canonical_string(&report).unwrap()),
            json_golden,
            "{scenario} JSON report"
        );
        let text_golden = fs::read_to_string(golden_path(scenario, "txt"))
            .unwrap_or_else(|error| panic!("{scenario}: {error}"));
        assert_eq!(render_text(&report), text_golden, "{scenario} text report");
    }
}
