// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! Documents written by the real IDApTIK corridor runner
//! (`idaptik-weevil-replay`, Evil Weevil kernel, ABI 1.1), vendored by
//! digest from IDApTIK `crates/idaptik-weevil/tests/fixtures/enaction` at
//! the commit in `vendor/idaptik/PROVENANCE.json`. IDApTIK's README there
//! says every pair exits 0 from `enaction-validate`; that is IDApTIK's
//! claim and is not re-checked here (no validator runs in this test).
//!
//! Kernel contract 2.0 (enaction-engine ADR-0030): the runner outputs are
//! `enaction-runner-output/2`, and each supersession link names its cause.
//! `every_corridor_runner_output_is_contract_2_and_breaks_no_link_rule`
//! holds the vendored set to that.
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
use ums_profile_sdk::explain::{check_links, explain, model_timelines, render_text};

/// The runs before slice 2, under the depth-0 policies.
const SCENARIOS: [&str; 6] = [
    "decoy-lure",
    "cautious-sentry",
    "the-grudge",
    "the-grudge-control",
    "the-flinch",
    "the-flinch-control",
];

/// Slice 2, "it models you": runs whose runner outputs hold models
/// (order-1 beliefs), under the depth-1 policies.
const MODEL_SCENARIOS: [&str; 6] = [
    "the-stalker",
    "the-stalker-control",
    "caught",
    "caught-control",
    "false-exposure",
    "false-exposure-control",
];

fn all_scenarios() -> impl Iterator<Item = &'static str> {
    SCENARIOS.into_iter().chain(MODEL_SCENARIOS)
}

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
fn the_vendored_model_policies_match_their_pinned_digest_and_allow_order_1() {
    let record = read_json(&provenance());
    let path = repo_root().join("vendor/idaptik/enaction-policies-models.json");
    let bytes = fs::read(&path).unwrap();
    assert_eq!(
        json!(sha256_hex(&bytes)),
        record["policies_models"]["sha256"],
        "vendor/idaptik/enaction-policies-models.json"
    );
    assert_eq!(read_json(&path)["depth"]["max_order"], 1);
    let depth_0 = read_json(&repo_root().join("vendor/idaptik/enaction-policies.json"));
    assert_eq!(depth_0["depth"]["max_order"], 0);
}

#[test]
fn every_scenario_has_its_runner_output_and_the_mind_scenarios_a_trace() {
    let dir = corridor();
    let mut runs = 0;
    for scenario in all_scenarios() {
        runs += 1;
        assert!(
            dir.join(format!("{scenario}.runner-output.json")).is_file(),
            "{scenario}"
        );
        let has_trace = dir.join(format!("{scenario}.mind-trace.json")).is_file();
        let ee_tick_only = matches!(scenario, "decoy-lure" | "cautious-sentry");
        assert_eq!(has_trace, !ee_tick_only, "{scenario}");
    }
    assert_eq!(runs, 12);
    // 12 runner outputs and 10 mind traces, plus README.md and SHA256SUMS.
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 24);
}

#[test]
fn the_model_runs_hold_models_and_the_earlier_runs_do_not() {
    let order = |scenario: &str| -> usize {
        let document = read_json(&corridor().join(format!("{scenario}.runner-output.json")));
        document["package"]["beliefs"]
            .as_array()
            .map(|beliefs| {
                beliefs
                    .iter()
                    .map(|belief| belief["holders"].as_array().map_or(0, Vec::len))
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    };
    for scenario in SCENARIOS {
        assert_eq!(order(scenario), 0, "{scenario}");
    }
    for scenario in ["the-stalker", "caught", "false-exposure"] {
        assert_eq!(order(scenario), 1, "{scenario}");
    }
    for scenario in [
        "the-stalker-control",
        "caught-control",
        "false-exposure-control",
    ] {
        assert_eq!(order(scenario), 0, "{scenario}");
    }
}

#[test]
fn every_corridor_runner_output_is_contract_2_and_breaks_no_link_rule() {
    // The lens's copy of belief-key-chained and the six cause rules finds
    // nothing in any corridor run, and every model run has at least one
    // link, so the check has something to read.
    let mut checked = 0;
    for scenario in all_scenarios() {
        let runner_output = read_json(&corridor().join(format!("{scenario}.runner-output.json")));
        assert_eq!(
            runner_output["contract"], "enaction-runner-output/2",
            "{scenario}"
        );
        check_links(&runner_output).unwrap_or_else(|findings| panic!("{scenario}: {findings:?}"));
        checked += 1;
    }
    assert_eq!(checked, 12);
    for scenario in ["the-stalker", "caught", "false-exposure"] {
        let runner_output = read_json(&corridor().join(format!("{scenario}.runner-output.json")));
        let links = runner_output["package"]["beliefs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|belief| belief.get("supersedes").is_some())
            .count();
        assert!(links > 0, "{scenario} has no supersession link");
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
    for scenario in all_scenarios() {
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

/// `(belief_id, [(from_tick, verdict)], until, stale_divergent)` for every
/// model of a scenario, from the conformance-shaped derivation.
type Model = (u64, Vec<(u64, &'static str)>, Option<u64>, bool);

fn models(scenario: &str) -> Vec<Model> {
    let document = read_json(&corridor().join(format!("{scenario}.runner-output.json")));
    let derived = model_timelines(&document).unwrap_or_else(|error| panic!("{scenario}: {error}"));
    derived["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|model| {
            let timeline = model["timeline"]
                .as_array()
                .unwrap()
                .iter()
                .map(|segment| {
                    let verdict = match segment["verdict"].as_str().unwrap() {
                        "agrees" => "agrees",
                        "diverges" => "diverges",
                        "no-claim" => "no-claim",
                        other => panic!("{scenario}: verdict {other}"),
                    };
                    (segment["from_tick"].as_u64().unwrap(), verdict)
                })
                .collect();
            (
                model["belief_id"].as_u64().unwrap(),
                timeline,
                model["until"].as_u64(),
                model["stale_divergent"].as_bool().unwrap(),
            )
        })
        .collect()
}

#[test]
fn false_exposure_has_a_model_that_agrees_from_21_and_diverges_from_52() {
    // The guard thinks the contact knows where it is from tick 21 (belief 3,
    // certain) and does not restate that until tick 112. The contact sees
    // the guard on tick 21 (belief 2) and, with a memory of 30 ticks
    // (IDApTIK 802b009), stops knowing on tick 52 (belief 4, unknown).
    assert_eq!(
        models("false-exposure"),
        vec![
            (1, vec![(1, "agrees")], Some(21), false),
            (3, vec![(21, "agrees"), (52, "diverges")], Some(112), true),
            (5, vec![(112, "agrees")], None, false),
        ]
    );
    let report = report("false-exposure");
    assert_eq!(
        report["model_summary"],
        json!({"models": 3, "stale_divergent": 1, "refused": 0})
    );
    let text = render_text(&report);
    for line in [
        "model 3: guard-0 models contact-41: \"guard-0 in room-0\" (certain 10000..10000: says it holds) [tested]",
        "  lifetime: from tick 21 until tick 112; supersedes belief 1",
        "    belief 2: certain (holds) ticks 21..51; agrees-with-world",
        "    belief 4: unknown (does not hold) ticks 52..; no-claim",
        "  ticks 21..51: agrees",
        "  ticks 52..111: diverges",
        "  stale-divergent: right when stated, wrong from tick 52 without being restated [tested]",
    ] {
        assert!(
            text.contains(&format!("{line}\n")),
            "missing {line:?}\n{text}"
        );
    }
}

#[test]
fn the_stalker_models_are_right_throughout() {
    assert_eq!(
        models("the-stalker"),
        vec![
            (1, vec![(1, "agrees")], Some(161), false),
            (3, vec![(161, "agrees")], None, false),
        ]
    );
}

#[test]
fn caught_has_a_model_that_agrees_from_101_to_the_end() {
    // The contact glances at tick 101 and, with a memory of 30 ticks
    // (IDApTIK 802b009), still knows when the guard's blows land from 116:
    // its belief is certain from 101 on, so the guard's model is right
    // throughout and not stale-divergent.
    assert_eq!(
        models("caught"),
        vec![
            (1, vec![(1, "agrees")], Some(101), false),
            (3, vec![(101, "agrees")], None, false),
        ]
    );
    assert_eq!(
        report("caught")["model_summary"],
        json!({"models": 2, "stale_divergent": 0, "refused": 0})
    );
}

#[test]
fn the_controls_hold_no_model() {
    for scenario in [
        "the-stalker-control",
        "caught-control",
        "false-exposure-control",
    ] {
        assert!(models(scenario).is_empty(), "{scenario}");
        let report = report(scenario);
        assert_eq!(report["models"], json!([]), "{scenario}");
        assert!(
            render_text(&report).contains("\nmodels: 0; stale-divergent: 0; refused: 0\n"),
            "{scenario}"
        );
    }
}

#[test]
fn a_corridor_model_is_never_compared_with_world_truth() {
    for scenario in ["the-stalker", "caught", "false-exposure"] {
        let original = read_json(&corridor().join(format!("{scenario}.runner-output.json")));
        let expected = model_timelines(&original).unwrap();
        let mut emptied = original.clone();
        emptied["world_truth"] = json!([]);
        assert_eq!(model_timelines(&emptied).unwrap(), expected, "{scenario}");
    }
}

#[test]
fn the_quickstart_excerpt_is_the_decoy_lure_golden_report() {
    // docs/DESIGNER-QUICKSTART.adoc, section 3, says its excerpt is pinned
    // byte for byte as decoy-lure.report.golden.txt. This test holds it to
    // that: the first [source,text] block after the section heading must
    // equal the golden exactly.
    let page = fs::read_to_string(repo_root().join("docs/DESIGNER-QUICKSTART.adoc")).unwrap();
    let section = page
        .split("== 3. What the report says")
        .nth(1)
        .expect("section 3");
    let block = section
        .split("[source,text]\n----\n")
        .nth(1)
        .and_then(|rest| rest.split("\n----\n").next())
        .expect("a [source,text] block");
    let golden = fs::read_to_string(golden_path("decoy-lure", "txt")).unwrap();
    assert_eq!(format!("{block}\n"), golden);
}

#[test]
fn the_adapter_page_excerpt_is_a_block_of_the_false_exposure_golden() {
    // docs/ENACTION-ADAPTER.adoc ("Models") quotes model 3 of the
    // false-exposure text report. The quote must appear in the golden
    // verbatim, so it cannot drift from the vendored documents.
    let page = fs::read_to_string(repo_root().join("docs/ENACTION-ADAPTER.adoc")).unwrap();
    let section = page
        .split("For IDApTIK's `false-exposure` run the text report shows:")
        .nth(1)
        .expect("the false-exposure excerpt");
    let block = section
        .split("[source,text]\n----\n")
        .nth(1)
        .and_then(|rest| rest.split("\n----\n").next())
        .expect("a [source,text] block");
    assert!(block.starts_with("model 3: "), "{block}");
    let golden = fs::read_to_string(golden_path("false-exposure", "txt")).unwrap();
    assert!(
        golden.contains(&format!("\n{block}\n")),
        "the page's excerpt is not in false-exposure.report.golden.txt:\n{block}"
    );
}
