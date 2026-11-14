// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! Stub-binary protocol tests for `ProcessPreviewAdapter` (ADR-0019).
//!
//! The stubs under `tests/fixtures/preview` are POSIX shell scripts that use
//! builtins only. They exist for tests; the adapter itself has no stub and no
//! fallback.

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use ums_profile_sdk::SimulationAdapter;
use ums_profile_sdk::preview::{PreviewError, ProcessPreviewAdapter};

fn stub(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/preview")
        .join(name)
}

fn adapter(runner: &str, validator: &str) -> ProcessPreviewAdapter {
    ProcessPreviewAdapter::new(stub(runner), stub(validator)).with_timeout(Duration::from_secs(10))
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("process-preview");
    fs::create_dir_all(&dir).expect("scratch dir");
    dir.join(name)
}

#[test]
fn deterministic_runner_and_accepting_validator_produce_a_response() {
    let outcome = adapter("runner-deterministic.sh", "validator-accepts.sh")
        .run(None)
        .expect("preview succeeds");
    let response = outcome.response().expect("response");
    assert_eq!(
        response["adapterContract"],
        "ums-enaction-preview-response/1"
    );
    assert_eq!(response["replay"], "identical");
    assert_eq!(response["validation"]["exit"], 0);
    let documents = response["documents"].as_array().expect("documents");
    assert_eq!(documents.len(), 2);
    assert!(documents[0].get("world_truth").is_some());
    assert_eq!(documents[1]["contract"], "enaction-mind-trace/1");
}

#[test]
fn missing_runner_binary_fails_the_preview() {
    let error = adapter("does-not-exist.sh", "validator-accepts.sh")
        .run(None)
        .expect_err("missing runner must fail");
    assert!(
        matches!(error, PreviewError::MissingBinary { .. }),
        "{error}"
    );
    assert!(error.to_string().contains("does-not-exist.sh"), "{error}");
}

#[test]
fn missing_validator_binary_fails_before_the_runner_runs() {
    let error = adapter("runner-deterministic.sh", "no-validator.sh")
        .run(None)
        .expect_err("missing validator must fail");
    assert!(
        matches!(error, PreviewError::MissingBinary { .. }),
        "{error}"
    );
    assert_eq!(error.kind(), "MissingBinary");
}

#[test]
fn non_executable_binary_fails_the_preview() {
    let path = scratch("not-executable.sh");
    fs::write(&path, "#!/bin/sh\nexit 0\n").expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
    }
    let error = ProcessPreviewAdapter::new(path, stub("validator-accepts.sh"))
        .run(None)
        .expect_err("non-executable runner must fail");
    assert!(
        matches!(error, PreviewError::NotExecutable { .. }),
        "{error}"
    );
}

#[test]
fn bare_command_name_is_refused_rather_than_searched_on_path() {
    let error = ProcessPreviewAdapter::new(PathBuf::from("sh"), stub("validator-accepts.sh"))
        .run(None)
        .expect_err("a bare name must not be resolved through PATH");
    assert!(
        matches!(error, PreviewError::NotExplicitPath { .. }),
        "{error}"
    );
}

#[test]
fn nondeterministic_runner_is_refused_with_replay_mismatch() {
    let error = adapter("runner-nondeterministic.sh", "validator-accepts.sh")
        .run(None)
        .expect_err("two differing runs must be refused");
    assert!(
        matches!(error, PreviewError::ReplayMismatch { .. }),
        "{error}"
    );
    assert!(error.to_string().starts_with("ReplayMismatch"), "{error}");
}

#[test]
fn validator_exit_1_is_a_refusal_that_names_rule_and_path() {
    let error = adapter("runner-deterministic.sh", "validator-refuses.sh")
        .run(None)
        .expect_err("validator refusal must refuse the preview");
    match &error {
        PreviewError::ValidatorRefused { rule, path, .. } => {
            assert_eq!(rule.as_deref(), Some("UnexplainedSelection"));
            assert_eq!(path.as_deref(), Some("/ticks/0/conation/selected"));
        }
        other => panic!("expected ValidatorRefused, got {other}"),
    }
    let text = error.to_string();
    assert!(text.contains("UnexplainedSelection"), "{text}");
    assert!(text.contains("/ticks/0/conation/selected"), "{text}");
}

#[test]
fn validator_receives_the_runner_output_on_stdin() {
    // validator-accepts.sh refuses (exit 1, rule NoRunnerOutput) unless its
    // first stdin line carries a runner-output package.
    let error = adapter("runner-argv.sh", "validator-accepts.sh")
        .run(None)
        .expect_err("input without a package is refused by the stub");
    match error {
        PreviewError::ValidatorRefused { rule, .. } => {
            assert_eq!(rule.as_deref(), Some("NoRunnerOutput"));
        }
        other => panic!("expected ValidatorRefused, got {other}"),
    }
}

#[test]
fn validator_operational_failure_is_not_reported_as_a_refusal() {
    let error = adapter("runner-deterministic.sh", "validator-broken.sh")
        .run(None)
        .expect_err("exit 2 must fail");
    assert!(
        matches!(error, PreviewError::ValidatorFailed { .. }),
        "{error}"
    );
    assert!(error.to_string().contains("missing --policies"), "{error}");
}

#[test]
fn failing_runner_fails_the_preview_with_its_diagnostic() {
    let error = adapter("runner-fails.sh", "validator-accepts.sh")
        .run(None)
        .expect_err("runner exit 3 must fail");
    assert!(
        matches!(error, PreviewError::RunnerFailed { .. }),
        "{error}"
    );
    assert!(
        error.to_string().contains("cannot load scenario"),
        "{error}"
    );
}

#[test]
fn hanging_runner_is_timed_out() {
    let error = ProcessPreviewAdapter::new(stub("runner-hangs.sh"), stub("validator-accepts.sh"))
        .with_timeout(Duration::from_millis(300))
        .run(None)
        .expect_err("must time out");
    assert!(matches!(error, PreviewError::Timeout { .. }), "{error}");
}

#[test]
fn flooding_runner_hits_the_stdout_cap() {
    let error = adapter("runner-floods.sh", "validator-accepts.sh")
        .with_stdout_cap(4096)
        .run(None)
        .expect_err("must be capped");
    assert!(
        matches!(error, PreviewError::OutputTooLarge { .. }),
        "{error}"
    );
}

fn documents_of(adapter: &ProcessPreviewAdapter, stdin: Option<&[u8]>) -> Vec<Value> {
    adapter
        .run(stdin)
        .expect("preview succeeds")
        .documents()
        .expect("documents")
}

#[test]
fn environment_is_cleared_for_the_runner() {
    // runner-env.sh prints env_leaked 1 if HOME or CARGO, which cargo sets
    // for this test process, are visible to it.
    assert!(std::env::var_os("HOME").is_some() || std::env::var_os("CARGO").is_some());
    let documents = documents_of(&adapter("runner-env.sh", "validator-accepts-any.sh"), None);
    assert_eq!(documents, vec![json!({"env_leaked": 0})]);
}

#[test]
fn runner_arguments_are_passed_as_an_argument_array_without_a_shell() {
    let documents = documents_of(
        &adapter("runner-argv.sh", "validator-accepts-any.sh").with_runner_args([
            "--emit-epistemic",
            "a;b",
            "$(x)",
            "two words",
        ]),
        None,
    );
    assert_eq!(
        documents,
        vec![json!({"argv": ["--emit-epistemic", "a;b", "$(x)", "two words"]})]
    );
}

#[test]
fn simulation_adapter_sends_a_preview_request_on_stdin() {
    let adapter = adapter("runner-echo-stdin.sh", "validator-accepts-any.sh")
        .with_profile("stub-profile", "0.0.0");
    let response = adapter
        .preview(&json!({"actors": []}), 31)
        .expect("trait preview succeeds");
    assert_eq!(
        response["documents"],
        json!([{
            "adapterContract": "ums-enaction-preview/1",
            "profile": "stub-profile",
            "profileVersion": "0.0.0",
            "seed": 31,
            "model": {"actors": []}
        }])
    );
    let refusing =
        ProcessPreviewAdapter::new(stub("runner-echo-stdin.sh"), stub("validator-refuses.sh"))
            .with_profile("stub-profile", "0.0.0");
    let error = refusing
        .preview(&json!({"actors": []}), 31)
        .expect_err("refusing validator fails the trait preview");
    assert!(error.contains("ValidatorRefused"), "{error}");
}

#[test]
fn simulation_adapter_without_a_profile_reference_fails() {
    let error = adapter("runner-deterministic.sh", "validator-accepts.sh")
        .preview(&json!({}), 1)
        .expect_err("no profile reference");
    assert!(error.contains("profile"), "{error}");
}

#[test]
fn runner_output_with_a_float_is_malformed() {
    let path = scratch("runner-float.sh");
    fs::write(&path, "#!/bin/sh\nprintf '{\"x\":1.5}\\n'\n").expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let outcome = ProcessPreviewAdapter::new(path, stub("validator-accepts-any.sh"))
        .run(None)
        .expect("preview runs");
    let error = outcome.documents().expect_err("float refused");
    assert!(
        matches!(error, PreviewError::MalformedRunnerOutput { .. }),
        "{error}"
    );
}

fn cli(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ums-profile"))
        .args(args)
        .env("UMS_PREVIEW_LEAK", "1")
        .output()
        .expect("ums-profile runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn cli_preview_prints_the_canonical_response() {
    let runner = stub("runner-deterministic.sh");
    let validator = stub("validator-accepts.sh");
    let (ok, stdout, stderr) = cli(&[
        "preview",
        "--runner",
        runner.to_str().unwrap(),
        "--validator",
        validator.to_str().unwrap(),
    ]);
    assert!(ok, "{stderr}");
    let response: Value = serde_json::from_str(stdout.trim()).expect("JSON response");
    assert_eq!(response["replay"], "identical");
}

#[test]
fn cli_preview_passes_hyphenated_runner_arguments() {
    let runner = stub("runner-argv.sh");
    let validator = stub("validator-refuses.sh");
    let (ok, _stdout, stderr) = cli(&[
        "preview",
        "--runner",
        runner.to_str().unwrap(),
        "--validator",
        validator.to_str().unwrap(),
        "--runner-arg",
        "--emit-mind-trace",
    ]);
    assert!(!ok);
    assert!(stderr.contains("ValidatorRefused"), "{stderr}");
    assert!(stderr.contains("UnexplainedSelection"), "{stderr}");
}

#[test]
fn cli_preview_refuses_a_nondeterministic_runner() {
    let runner = stub("runner-nondeterministic.sh");
    let validator = stub("validator-accepts.sh");
    let (ok, _stdout, stderr) = cli(&[
        "preview",
        "--runner",
        runner.to_str().unwrap(),
        "--validator",
        validator.to_str().unwrap(),
    ]);
    assert!(!ok);
    assert!(stderr.contains("ReplayMismatch"), "{stderr}");
}

#[test]
fn cli_preview_fails_on_a_missing_binary() {
    let validator = stub("validator-accepts.sh");
    let (ok, _stdout, stderr) = cli(&[
        "preview",
        "--runner",
        "/nonexistent/runner",
        "--validator",
        validator.to_str().unwrap(),
    ]);
    assert!(!ok);
    assert!(stderr.contains("MissingBinary"), "{stderr}");
}

#[test]
fn cli_preview_clears_the_environment() {
    // cli() sets UMS_PREVIEW_LEAK=1 in the CLI's own environment.
    let runner = stub("runner-env.sh");
    let validator = stub("validator-accepts-any.sh");
    let (ok, stdout, stderr) = cli(&[
        "preview",
        "--runner",
        runner.to_str().unwrap(),
        "--validator",
        validator.to_str().unwrap(),
    ]);
    assert!(ok, "{stderr}");
    let response: Value = serde_json::from_str(stdout.trim()).expect("JSON response");
    assert_eq!(response["documents"], json!([{"env_leaked": 0}]));
}

#[test]
fn cli_preview_passes_validator_arguments() {
    let runner = stub("runner-deterministic.sh");
    let validator = stub("runner-argv.sh");
    // runner-argv.sh as a validator prints its argv and exits 0.
    let (ok, stdout, stderr) = cli(&[
        "preview",
        "--runner",
        runner.to_str().unwrap(),
        "--validator",
        validator.to_str().unwrap(),
        "--validator-arg",
        "--expect-profile",
        "--validator-arg",
        "stub@0.0.0",
    ]);
    assert!(ok, "{stderr}");
    let response: Value = serde_json::from_str(stdout.trim()).expect("JSON response");
    assert_eq!(
        response["validation"]["stdout"],
        json!("{\"argv\":[\"--expect-profile\",\"stub@0.0.0\"]}\n")
    );
}
