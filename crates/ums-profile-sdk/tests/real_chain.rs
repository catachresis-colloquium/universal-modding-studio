// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! The preview against the REAL binaries: `idaptik-weevil-replay` (IDApTIK,
//! with the Evil Weevil kernel) and `enaction-validate` (enaction-engine).
//!
//! Every test here is `#[ignore]`d, because the binaries are built outside
//! this repository and are not in its CI. Run them by name:
//!
//! ```text
//! ENACTION_VALIDATE=/path/to/enaction-validate \
//! IDAPTIK_WEEVIL_REPLAY=/path/to/idaptik-weevil-replay \
//! cargo test --locked -p ums-profile-sdk --test real_chain -- --ignored --nocapture
//! ```
//!
//! With either variable unset each test FAILS with a message naming the
//! variable; none passes silently. The policies are the corridor's own,
//! vendored at `vendor/idaptik/enaction-policies.json`.
//!
//! Expected figures (measured by the coordinator on the real chain on
//! 2026-11-14): decoy-lure, 200 ticks, seed 7, has 36 beliefs and every one
//! diverges from the world; cautious-sentry has 36 and every one agrees.
//!
//! The six slice 2 model scenarios (the-stalker, caught, false-exposure
//! and their controls) hold models, beliefs of order 1. They are previewed
//! under the depth-1 policies `vendor/idaptik/enaction-policies-models.json`;
//! under the depth-0 file the real validator must refuse them as
//! `nesting-bounded`. Their figures are those of the vendored documents
//! (IDApTIK pull request 234 head d867eca, kernel contract 2.0, where each
//! contact remembers for 30 ticks). The move to contract 2.0 changed the
//! tag and the supersession links, and no count, verdict or timeline. The
//! coordinator ran all 8 tests on 2026-11-15 against IDApTIK 462af7c; this
//! package has not run them against d867eca, and the validator must be one
//! that reads kernel contract 2.0 (enaction-engine 11bd247 or later).

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use support::repo_root;
use ums_profile_sdk::explain::explain;
use ums_profile_sdk::preview::{
    MIND_TRACE_SLOT, PreviewError, ProcessPreviewAdapter, RUNNER_OUTPUT_SLOT,
};

struct Binaries {
    validator: PathBuf,
    runner: PathBuf,
}

fn binaries() -> Binaries {
    let get = |name: &str, what: &str| -> PathBuf {
        match std::env::var_os(name) {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => panic!(
                "real_chain: {name} is not set. Set {name} to the path of {what}. \
                 This test is #[ignore]d and needs both ENACTION_VALIDATE and \
                 IDAPTIK_WEEVIL_REPLAY; it does not pass without them."
            ),
        }
    };
    Binaries {
        validator: get(
            "ENACTION_VALIDATE",
            "enaction-validate (enaction-engine: cargo build --locked -p enaction-contracts --bin enaction-validate)",
        ),
        runner: get(
            "IDAPTIK_WEEVIL_REPLAY",
            "idaptik-weevil-replay (IDApTIK: cargo build --locked -p idaptik-weevil --bin idaptik-weevil-replay)",
        ),
    }
}

fn policies() -> PathBuf {
    repo_root().join("vendor/idaptik/enaction-policies.json")
}

/// The depth-1 policies of the slice 2 model scenarios.
fn model_policies() -> PathBuf {
    repo_root().join("vendor/idaptik/enaction-policies-models.json")
}

const MODEL_SCENARIOS: [&str; 6] = [
    "the-stalker",
    "the-stalker-control",
    "caught",
    "caught-control",
    "false-exposure",
    "false-exposure-control",
];

fn corridor(name: &str) -> PathBuf {
    repo_root()
        .join("vendor/idaptik/weevil-corridor")
        .join(name)
}

/// `ums-profile preview --explain` for one corridor profile, through the
/// CLI a designer uses. Returns stdout; fails the test on a non-zero exit.
fn cli_preview(profile: &str, mind: bool) -> String {
    cli_preview_with(profile, mind, &policies())
}

fn cli_preview_with(profile: &str, mind: bool, policies: &Path) -> String {
    let bin = binaries();
    let mut args: Vec<String> = vec![
        "preview".into(),
        "--runner".into(),
        bin.runner.display().to_string(),
        "--validator".into(),
        bin.validator.display().to_string(),
        "--policies".into(),
        policies.display().to_string(),
        "--runner-output-flag".into(),
        "--emit-epistemic".into(),
    ];
    if mind {
        args.extend(["--mind-trace-flag".into(), "--emit-mind-trace".into()]);
    }
    for arg in ["--profile", profile, "--ticks", "200", "--seed", "7"] {
        args.extend(["--runner-arg".into(), arg.into()]);
    }
    args.push("--explain".into());
    let output = Command::new(env!("CARGO_BIN_EXE_ums-profile"))
        .args(&args)
        .output()
        .expect("ums-profile runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("$ ums-profile {}\n{stdout}", args.join(" "));
    assert!(
        output.status.success(),
        "{profile}: preview failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    stdout
}

#[test]
#[ignore = "needs the real enaction-validate and idaptik-weevil-replay (ENACTION_VALIDATE, IDAPTIK_WEEVIL_REPLAY)"]
fn decoy_lure_previews_with_every_belief_diverging() {
    let stdout = cli_preview("decoy-lure", false);
    assert!(stdout.starts_with("replay-identical: ok\n"), "{stdout}");
    assert!(stdout.contains("validator: accepted (exit 0)"), "{stdout}");
    assert!(stdout.contains("(validator-accepted)"), "{stdout}");
    assert!(
        stdout.contains(
            "\nbeliefs: 36; agrees-with-world: 0; diverges-from-world: 36; no-claim: 0\n"
        ),
        "decoy-lure: expected 36 of 36 diverging\n{stdout}"
    );
}

#[test]
#[ignore = "needs the real enaction-validate and idaptik-weevil-replay (ENACTION_VALIDATE, IDAPTIK_WEEVIL_REPLAY)"]
fn cautious_sentry_previews_with_every_belief_agreeing() {
    let stdout = cli_preview("cautious-sentry", false);
    assert!(stdout.starts_with("replay-identical: ok\n"), "{stdout}");
    assert!(
        stdout.contains(
            "\nbeliefs: 36; agrees-with-world: 36; diverges-from-world: 0; no-claim: 0\n"
        ),
        "cautious-sentry: expected 36 of 36 agreeing\n{stdout}"
    );
}

/// Package B2's done_when, through the command a designer runs: the
/// preview of "the grudge" prints `replay-identical: ok`, reports at least
/// one commitment-override tick, and matches its golden report. The golden
/// is rendered from the vendored (unvalidated) documents, so the CLI form
/// differs only in its two header lines and the validation state.
#[test]
#[ignore = "needs the real enaction-validate and idaptik-weevil-replay (ENACTION_VALIDATE, IDAPTIK_WEEVIL_REPLAY)"]
fn the_grudge_cli_preview_matches_its_golden_report() {
    let stdout = cli_preview("the-grudge", true);
    let golden = fs::read_to_string(repo_root().join(
        "crates/ums-profile-sdk/tests/fixtures/weevil-corridor/the-grudge.report.golden.txt",
    ))
    .expect("the-grudge text golden");
    assert_eq!(
        golden.matches("(not-validated)").count(),
        1,
        "the golden names its validation state exactly once"
    );
    let expected = format!(
        "replay-identical: ok\nvalidator: accepted (exit 0)\n{}",
        golden.replacen("(not-validated)", "(validator-accepted)", 1)
    );
    assert!(
        stdout.contains("commitment-override ticks: 37 of 200"),
        "{stdout}"
    );
    assert_eq!(
        stdout, expected,
        "the-grudge CLI report differs from its golden"
    );
}

/// The library adapter with both documents, for the mind scenarios.
fn adapter(profile: &str) -> ProcessPreviewAdapter {
    adapter_with(profile, policies())
}

fn adapter_with(profile: &str, policies: PathBuf) -> ProcessPreviewAdapter {
    let bin = binaries();
    ProcessPreviewAdapter::new(bin.runner, bin.validator)
        .with_policies(policies)
        .with_runner_args([
            "--profile",
            profile,
            "--ticks",
            "200",
            "--seed",
            "7",
            "--emit-epistemic",
            RUNNER_OUTPUT_SLOT,
            "--emit-mind-trace",
            MIND_TRACE_SLOT,
        ])
}

fn same_as_vendored(profile: &str, document: &str, bytes: &[u8]) {
    let vendored = corridor(&format!("{profile}.{document}.json"));
    let expected = fs::read(&vendored).expect("vendored document");
    assert!(
        bytes == expected.as_slice(),
        "{profile}: the real runner's {document} differs from {}; re-vendor it (vendor/idaptik/PROVENANCE.json) or find the change",
        vendored.display()
    );
}

#[test]
#[ignore = "needs the real enaction-validate and idaptik-weevil-replay (ENACTION_VALIDATE, IDAPTIK_WEEVIL_REPLAY)"]
fn the_grudge_and_the_flinch_match_their_vendored_documents() {
    for profile in [
        "the-grudge",
        "the-grudge-control",
        "the-flinch",
        "the-flinch-control",
    ] {
        let outcome = adapter(profile)
            .run(None)
            .unwrap_or_else(|error| panic!("{profile}: {error}"));
        same_as_vendored(profile, "runner-output", &outcome.runner_output);
        same_as_vendored(
            profile,
            "mind-trace",
            outcome.mind_trace.as_deref().expect("mind trace"),
        );
        let report = explain(
            &outcome.runner_output_document().unwrap(),
            outcome.mind_trace_document().unwrap().as_ref(),
            true,
        )
        .unwrap();
        let guard = report["agents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|agent| agent["agent"] == "guard-0")
            .expect("guard-0")
            .clone();
        println!(
            "{profile}: replay-identical: ok; commitment-override ticks {}; startle at {}",
            guard["commitment_override_ticks"], guard["tells_at"]["startle"]
        );
        match profile {
            "the-grudge" => assert_eq!(guard["commitment_override_ticks"], 37),
            "the-grudge-control" | "the-flinch" | "the-flinch-control" => {
                assert_eq!(guard["commitment_override_ticks"], 0, "{profile}")
            }
            _ => unreachable!(),
        }
        if profile == "the-flinch" {
            assert_eq!(guard["tells_at"]["startle"], serde_json::json!([10]));
        }
        if profile == "the-flinch-control" {
            assert!(guard["tells_at"].get("startle").is_none());
        }
    }
}

#[cfg(unix)]
fn executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, body).expect("write");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// A runner that copies `content` into its runner-output slot, with shell
/// builtins only (the adapter clears PATH).
#[cfg(unix)]
fn copying_runner(name: &str, content: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("real-chain");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join(format!("{name}.runner-output.json"));
    fs::write(&source, content).unwrap();
    let runner = dir.join(format!("emit-{name}.sh"));
    executable(
        &runner,
        &format!(
            "#!/bin/sh\nwhile IFS= read -r line || [ -n \"$line\" ]; do printf '%s\\n' \"$line\"; done < '{}' > \"$1\"\n",
            source.display()
        ),
    );
    runner
}

#[test]
#[cfg(unix)]
#[ignore = "needs the real enaction-validate (ENACTION_VALIDATE; IDAPTIK_WEEVIL_REPLAY is also required, for uniformity)"]
fn a_tampered_runner_output_is_refused_by_the_real_validator_with_the_digest_rule() {
    let bin = binaries();
    let original = fs::read_to_string(corridor("decoy-lure.runner-output.json")).unwrap();
    let preview = |runner: PathBuf| {
        ProcessPreviewAdapter::new(runner, bin.validator.clone())
            .with_policies(policies())
            .with_runner_args([RUNNER_OUTPUT_SLOT])
            .run(None)
    };
    // The untampered real document is accepted through the same path, so a
    // refusal below is caused by the tamper.
    preview(copying_runner("clean", &original))
        .unwrap_or_else(|error| panic!("the clean decoy-lure document was refused: {error}"));

    // One belief's interval changed and the digest left as it was. 6001 is
    // inside the same status band as 6000 (suspected, 5000..7000), so the
    // stale digest is the defect.
    let marker = "\"confidence\":{\"belief\":6000,";
    assert!(
        original.contains(marker),
        "the vendored document changed shape"
    );
    let tampered = original.replacen(marker, "\"confidence\":{\"belief\":6001,", 1);
    let error = preview(copying_runner("tampered", &tampered))
        .expect_err("the real validator must refuse a stale digest");
    println!("{error}");
    match &error {
        PreviewError::ValidatorRefused { rule, path, .. } => {
            assert_eq!(rule.as_deref(), Some("digest-matches"), "{error}");
            assert_eq!(path.as_deref(), Some("package.manifest.digest"), "{error}");
        }
        other => panic!("expected ValidatorRefused, got {other}"),
    }
}

/// The model counts of the vendored slice 2 documents: (models,
/// stale-divergent). Pinned by `tests/vendored_idaptik.rs` on the vendored
/// bytes; here the real runner must reproduce those bytes.
fn expected_models(profile: &str) -> (u64, u64) {
    match profile {
        "the-stalker" => (2, 0),
        "caught" => (2, 0),
        "false-exposure" => (3, 1),
        "the-stalker-control" | "caught-control" | "false-exposure-control" => (0, 0),
        other => panic!("not a model scenario: {other}"),
    }
}

#[test]
#[ignore = "needs the real enaction-validate and idaptik-weevil-replay (ENACTION_VALIDATE, IDAPTIK_WEEVIL_REPLAY)"]
fn the_model_scenarios_match_their_vendored_documents_under_the_depth_1_policies() {
    for profile in MODEL_SCENARIOS {
        let outcome = adapter_with(profile, model_policies())
            .run(None)
            .unwrap_or_else(|error| panic!("{profile}: {error}"));
        same_as_vendored(profile, "runner-output", &outcome.runner_output);
        same_as_vendored(
            profile,
            "mind-trace",
            outcome.mind_trace.as_deref().expect("mind trace"),
        );
        let report = explain(
            &outcome.runner_output_document().unwrap(),
            outcome.mind_trace_document().unwrap().as_ref(),
            true,
        )
        .unwrap();
        println!(
            "{profile}: replay-identical: ok; validator accepted; {}",
            report["model_summary"]
        );
        assert_eq!(
            report["inputs"]["runner_output"], "enaction-runner-output/2",
            "{profile}"
        );
        let (models, stale) = expected_models(profile);
        assert_eq!(report["model_summary"]["models"], models, "{profile}");
        assert_eq!(
            report["model_summary"]["stale_divergent"], stale,
            "{profile}"
        );
        assert_eq!(report["model_summary"]["refused"], 0, "{profile}");
    }
}

#[test]
#[ignore = "needs the real enaction-validate and idaptik-weevil-replay (ENACTION_VALIDATE, IDAPTIK_WEEVIL_REPLAY)"]
fn a_model_scenario_is_refused_as_nesting_bounded_under_the_depth_0_policies() {
    for profile in ["the-stalker", "caught", "false-exposure"] {
        let error = adapter(profile)
            .run(None)
            .expect_err("an order-1 package under depth 0 must be refused");
        println!("{profile}: {error}");
        match &error {
            PreviewError::ValidatorRefused { rule, .. } => {
                assert_eq!(
                    rule.as_deref(),
                    Some("nesting-bounded"),
                    "{profile}: {error}"
                );
            }
            other => panic!("{profile}: expected ValidatorRefused, got {other}"),
        }
    }
}

/// The designer's command for false-exposure, against its golden report:
/// the guard's model agrees on ticks 21 to 51 and diverges from tick 52,
/// marked stale-divergent.
#[test]
#[ignore = "needs the real enaction-validate and idaptik-weevil-replay (ENACTION_VALIDATE, IDAPTIK_WEEVIL_REPLAY)"]
fn the_false_exposure_cli_preview_matches_its_golden_report() {
    let stdout = cli_preview_with("false-exposure", true, &model_policies());
    let golden = fs::read_to_string(repo_root().join(
        "crates/ums-profile-sdk/tests/fixtures/weevil-corridor/false-exposure.report.golden.txt",
    ))
    .expect("false-exposure text golden");
    assert_eq!(golden.matches("(not-validated)").count(), 1);
    let expected = format!(
        "replay-identical: ok\nvalidator: accepted (exit 0)\n{}",
        golden.replacen("(not-validated)", "(validator-accepted)", 1)
    );
    assert!(
        stdout.contains("  stale-divergent: right when stated, wrong from tick 52 without being restated [tested]\n"),
        "{stdout}"
    );
    assert_eq!(
        stdout, expected,
        "false-exposure CLI report differs from its golden"
    );
}
