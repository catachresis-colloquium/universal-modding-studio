// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! Stub-binary protocol tests for `ProcessPreviewAdapter` (ADR-0019).
//!
//! The stubs under `tests/fixtures/preview` are POSIX shell scripts that use
//! builtins only. `runner-deterministic.sh` and `validator-accepts.sh` copy
//! the command lines of `idaptik-weevil-replay` and `enaction-validate`:
//! each document goes to its own file, the validator reads the runner output
//! on stdin and the mind trace from `--mind-trace FILE`, and `--policies
//! FILE` is required. They exist for tests; the adapter itself has no stub
//! and no fallback.

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use ums_profile_sdk::SimulationAdapter;
use ums_profile_sdk::preview::{
    MIND_TRACE_SLOT, PreviewError, ProcessPreviewAdapter, RUNNER_OUTPUT_SLOT,
};

fn stub(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/preview")
        .join(name)
}

fn policies() -> PathBuf {
    stub("policies.json")
}

/// The runner template of the real runner: both documents to files.
fn both_documents() -> Vec<&'static str> {
    vec![
        "--emit-epistemic",
        RUNNER_OUTPUT_SLOT,
        "--emit-mind-trace",
        MIND_TRACE_SLOT,
    ]
}

fn adapter(runner: &str, validator: &str) -> ProcessPreviewAdapter {
    ProcessPreviewAdapter::new(stub(runner), stub(validator))
        .with_policies(policies())
        .with_timeout(Duration::from_secs(10))
}

/// The real-shaped pair: both documents, accepting validator.
fn real_shaped(extra: &[&str]) -> ProcessPreviewAdapter {
    let mut args = both_documents();
    args.extend_from_slice(extra);
    adapter("runner-deterministic.sh", "validator-accepts.sh").with_runner_args(args)
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("process-preview");
    fs::create_dir_all(&dir).expect("scratch dir");
    dir.join(name)
}

#[cfg(unix)]
fn executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, body).expect("write");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[test]
fn both_documents_are_paired_by_file_and_validated() {
    let outcome = real_shaped(&[]).run(None).expect("preview succeeds");
    let response = outcome.response().expect("response");
    assert_eq!(
        response["adapterContract"],
        "ums-enaction-preview-response/2"
    );
    assert_eq!(response["replay"], "identical");
    assert_eq!(response["validation"]["exit"], 0);
    assert_eq!(
        response["runnerOutput"]["document"]["contract"],
        "enaction-runner-output/1"
    );
    assert_eq!(
        response["mindTrace"]["document"]["contract"],
        "enaction-mind-trace/1"
    );
    // The validator echoed both documents, one line each, which it does only
    // when it was given --mind-trace.
    let stdout = response["validation"]["stdout"].as_str().unwrap();
    assert_eq!(stdout.lines().count(), 2, "{stdout}");
    assert!(
        stdout
            .lines()
            .nth(1)
            .unwrap()
            .contains("enaction-mind-trace/1")
    );
}

#[test]
fn runner_that_emits_only_the_runner_output_is_previewed_without_a_mind_trace() {
    let outcome = adapter("runner-deterministic.sh", "validator-accepts.sh")
        .with_runner_args(["--emit-epistemic", RUNNER_OUTPUT_SLOT])
        .run(None)
        .expect("preview succeeds");
    assert!(outcome.mind_trace.is_none());
    let response = outcome.response().expect("response");
    assert!(response.get("mindTrace").is_none(), "{response}");
    // No --mind-trace was passed: the validator echoed the envelope only.
    let stdout = response["validation"]["stdout"].as_str().unwrap();
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
}

#[test]
fn runner_stdout_may_differ_between_runs_because_documents_are_compared() {
    // The stub, like idaptik-weevil-replay, prints "wrote PATH" on stdout,
    // and the two runs write into two different directories.
    let outcome = real_shaped(&[]).run(None).expect("preview succeeds");
    let stdout = String::from_utf8(outcome.runner_stdout.clone()).unwrap();
    assert!(stdout.contains("wrote /"), "{stdout}");
}

#[test]
fn the_two_runs_write_into_two_private_directories_that_are_removed() {
    let log = scratch("paths.log");
    let _ = fs::remove_file(&log);
    real_shaped(&["--log", log.to_str().unwrap()])
        .run(None)
        .expect("preview succeeds");
    let text = fs::read_to_string(&log).expect("log");
    let paths: Vec<PathBuf> = text.lines().map(PathBuf::from).collect();
    assert_eq!(paths.len(), 2, "{text}");
    assert!(paths.iter().all(|path| path.is_absolute()), "{text}");
    let first = paths[0].parent().unwrap();
    let second = paths[1].parent().unwrap();
    assert_ne!(first, second, "two runs, two directories");
    assert_eq!(first.parent(), second.parent(), "one private root");
    let root = first.parent().unwrap();
    assert!(!root.exists(), "{} was not removed", root.display());
    #[cfg(unix)]
    {
        // While it existed the root was private; that is checked by
        // creating one here through the same code path.
        let probe = ums_profile_sdk::preview::PrivateDir::create().expect("dir");
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(probe.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }
}

#[test]
fn differing_runner_outputs_are_refused_with_replay_mismatch() {
    let error = adapter("runner-nondeterministic.sh", "validator-accepts.sh")
        .with_runner_args(["--emit-epistemic", RUNNER_OUTPUT_SLOT])
        .run(None)
        .expect_err("two differing runs must be refused");
    match &error {
        PreviewError::ReplayMismatch { document, .. } => assert_eq!(*document, "runner-output"),
        other => panic!("expected ReplayMismatch, got {other}"),
    }
    assert!(error.to_string().starts_with("ReplayMismatch"), "{error}");
}

#[test]
fn differing_mind_traces_are_refused_with_replay_mismatch() {
    let error = real_shaped(&["--mode", "vary-mind"])
        .run(None)
        .expect_err("two differing mind traces must be refused");
    match &error {
        PreviewError::ReplayMismatch { document, .. } => assert_eq!(*document, "mind-trace"),
        other => panic!("expected ReplayMismatch, got {other}"),
    }
    assert!(error.to_string().contains("mind-trace"), "{error}");
}

#[test]
fn a_template_without_the_runner_output_slot_is_refused() {
    let error = adapter("runner-deterministic.sh", "validator-accepts.sh")
        .with_runner_args(["--emit-mind-trace", MIND_TRACE_SLOT])
        .run(None)
        .expect_err("no runner-output slot");
    assert!(matches!(error, PreviewError::BadRequest { .. }), "{error}");
    assert!(error.to_string().contains(RUNNER_OUTPUT_SLOT), "{error}");
}

#[test]
fn a_slot_inside_a_larger_argument_or_twice_is_refused() {
    for args in [
        vec!["--emit-epistemic={runner_output}"],
        vec![RUNNER_OUTPUT_SLOT, RUNNER_OUTPUT_SLOT],
        vec![RUNNER_OUTPUT_SLOT, MIND_TRACE_SLOT, MIND_TRACE_SLOT],
    ] {
        let error = adapter("runner-deterministic.sh", "validator-accepts.sh")
            .with_runner_args(args.clone())
            .run(None)
            .expect_err("ambiguous template");
        assert!(
            matches!(error, PreviewError::BadRequest { .. }),
            "{args:?}: {error}"
        );
    }
}

#[test]
fn a_document_the_runner_was_asked_for_but_did_not_write_fails() {
    let error = real_shaped(&["--mode", "skip-mind"])
        .run(None)
        .expect_err("missing mind trace");
    match &error {
        PreviewError::MissingDocument { run, document } => {
            assert_eq!((*run, *document), (1, "mind-trace"));
        }
        other => panic!("expected MissingDocument, got {other}"),
    }
    let error = real_shaped(&["--mode", "nothing"])
        .run(None)
        .expect_err("missing runner output");
    assert!(
        matches!(
            error,
            PreviewError::MissingDocument {
                document: "runner-output",
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn a_document_over_the_file_cap_is_refused() {
    let error = real_shaped(&["--mode", "big"])
        .with_document_cap(1024)
        .run(None)
        .expect_err("capped");
    match &error {
        PreviewError::DocumentTooLarge { document, cap, .. } => {
            assert_eq!((*document, *cap), ("runner-output", 1024));
        }
        other => panic!("expected DocumentTooLarge, got {other}"),
    }
    // The same run passes under the default cap.
    real_shaped(&["--mode", "big"])
        .run(None)
        .expect("under the default cap");
}

#[test]
fn policies_are_required_and_must_exist() {
    let error = ProcessPreviewAdapter::new(
        stub("runner-deterministic.sh"),
        stub("validator-accepts.sh"),
    )
    .with_runner_args(both_documents())
    .run(None)
    .expect_err("no policies");
    assert!(matches!(error, PreviewError::BadRequest { .. }), "{error}");
    assert!(error.to_string().contains("--policies"), "{error}");

    let error = real_shaped(&[])
        .with_policies("/nonexistent/policies.json")
        .run(None)
        .expect_err("missing policies file");
    assert!(
        matches!(error, PreviewError::MissingPolicies { .. }),
        "{error}"
    );
}

#[test]
fn missing_runner_binary_fails_the_preview() {
    let error = adapter("does-not-exist.sh", "validator-accepts.sh")
        .with_runner_args(both_documents())
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
    let log = scratch("never.log");
    let _ = fs::remove_file(&log);
    let error = adapter("runner-deterministic.sh", "no-validator.sh")
        .with_runner_args([
            "--emit-epistemic",
            RUNNER_OUTPUT_SLOT,
            "--log",
            log.to_str().unwrap(),
        ])
        .run(None)
        .expect_err("missing validator must fail");
    assert!(
        matches!(error, PreviewError::MissingBinary { .. }),
        "{error}"
    );
    assert!(
        !log.exists(),
        "the runner ran although the validator is missing"
    );
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
        .with_policies(policies())
        .with_runner_args(both_documents())
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
        .with_policies(policies())
        .with_runner_args(both_documents())
        .run(None)
        .expect_err("a bare name must not be resolved through PATH");
    assert!(
        matches!(error, PreviewError::NotExplicitPath { .. }),
        "{error}"
    );
}

#[test]
fn a_refusal_names_the_validators_rule_and_path() {
    // The stub refuses a runner output marked TAMPERED with the real
    // validator's digest rule, in the real one-line form.
    let error = real_shaped(&["--mode", "tamper"])
        .run(None)
        .expect_err("tampered runner output is refused");
    match &error {
        PreviewError::ValidatorRefused {
            rule, path, more, ..
        } => {
            assert_eq!(rule.as_deref(), Some("digest-matches"));
            assert_eq!(path.as_deref(), Some("package.manifest.digest"));
            assert_eq!(*more, 0);
        }
        other => panic!("expected ValidatorRefused, got {other}"),
    }
    let text = error.to_string();
    assert!(text.contains("digest-matches"), "{text}");
    assert!(text.contains("package.manifest.digest"), "{text}");
}

#[test]
fn a_refusal_with_several_faults_reports_the_first_and_counts_the_rest() {
    let error = adapter("runner-deterministic.sh", "validator-refuses.sh")
        .with_runner_args(both_documents())
        .run(None)
        .expect_err("refused");
    match &error {
        PreviewError::ValidatorRefused {
            rule,
            path,
            detail,
            more,
        } => {
            assert_eq!(rule.as_deref(), Some("UnexplainedSelection"));
            assert_eq!(
                path.as_deref(),
                Some("mind_trace.ticks[0].conation.selected")
            );
            assert!(detail.contains("neither first"), "{detail}");
            assert_eq!(*more, 1);
        }
        other => panic!("expected ValidatorRefused, got {other}"),
    }
    assert!(error.to_string().contains("1 more fault"), "{error}");
}

#[test]
fn validator_receives_the_runner_output_on_stdin() {
    // validator-accepts.sh refuses (UnknownField at contract) unless its
    // stdin is a runner output; runner-argv.sh writes an argv document.
    let error = adapter("runner-argv.sh", "validator-accepts.sh")
        .with_runner_args([RUNNER_OUTPUT_SLOT])
        .run(None)
        .expect_err("input that is not a runner output is refused");
    match error {
        PreviewError::ValidatorRefused { rule, path, .. } => {
            assert_eq!(rule.as_deref(), Some("UnknownField"));
            assert_eq!(path.as_deref(), Some("contract"));
        }
        other => panic!("expected ValidatorRefused, got {other}"),
    }
}

#[test]
fn validator_operational_failure_is_not_reported_as_a_refusal() {
    let error = adapter("runner-deterministic.sh", "validator-broken.sh")
        .with_runner_args(both_documents())
        .run(None)
        .expect_err("exit 2 must fail");
    assert!(
        matches!(error, PreviewError::ValidatorFailed { .. }),
        "{error}"
    );
    assert!(error.to_string().contains("missing --policies"), "{error}");
}

#[test]
fn extra_validator_arguments_reach_the_validator() {
    // The stub validator, like the real one, exits 2 on an unknown argument.
    let error = real_shaped(&[])
        .with_validator_args(["--expect-profile"])
        .run(None)
        .expect_err("unknown validator argument");
    assert!(
        matches!(error, PreviewError::ValidatorFailed { .. }),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .contains("unknown argument --expect-profile"),
        "{error}"
    );
}

#[test]
fn failing_runner_fails_the_preview_with_its_diagnostic() {
    let error = adapter("runner-fails.sh", "validator-accepts.sh")
        .with_runner_args(both_documents())
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
    let error = adapter("runner-hangs.sh", "validator-accepts.sh")
        .with_runner_args(both_documents())
        .with_timeout(Duration::from_millis(300))
        .run(None)
        .expect_err("must time out");
    assert!(matches!(error, PreviewError::Timeout { .. }), "{error}");
}

#[test]
fn flooding_runner_hits_the_stdout_cap() {
    let error = adapter("runner-floods.sh", "validator-accepts.sh")
        .with_runner_args(both_documents())
        .with_stdout_cap(4096)
        .run(None)
        .expect_err("must be capped");
    assert!(
        matches!(error, PreviewError::OutputTooLarge { .. }),
        "{error}"
    );
}

fn runner_document(adapter: &ProcessPreviewAdapter, stdin: Option<&[u8]>) -> Value {
    adapter
        .run(stdin)
        .expect("preview succeeds")
        .runner_output_document()
        .expect("document")
}

#[test]
fn environment_is_cleared_for_the_runner() {
    // runner-env.sh writes env_leaked 1 if HOME or CARGO, which cargo sets
    // for this test process, are visible to it.
    assert!(std::env::var_os("HOME").is_some() || std::env::var_os("CARGO").is_some());
    let document = runner_document(
        &adapter("runner-env.sh", "validator-accepts-any.sh")
            .with_runner_args([RUNNER_OUTPUT_SLOT]),
        None,
    );
    assert_eq!(document, json!({"env_leaked": 0}));
}

#[test]
fn runner_arguments_are_passed_as_an_argument_array_without_a_shell() {
    let document = runner_document(
        &adapter("runner-argv.sh", "validator-accepts-any.sh").with_runner_args([
            RUNNER_OUTPUT_SLOT,
            "--emit-epistemic",
            "a;b",
            "$(x)",
            "two words",
        ]),
        None,
    );
    assert_eq!(
        document,
        json!({"argv": ["--emit-epistemic", "a;b", "$(x)", "two words"]})
    );
}

#[test]
fn simulation_adapter_sends_a_preview_request_on_stdin() {
    let adapter = adapter("runner-echo-stdin.sh", "validator-accepts-any.sh")
        .with_runner_args([RUNNER_OUTPUT_SLOT])
        .with_profile("stub-profile", "0.0.0");
    let response = adapter
        .preview(&json!({"actors": []}), 31)
        .expect("trait preview succeeds");
    assert_eq!(
        response["runnerOutput"]["document"],
        json!({
            "adapterContract": "ums-enaction-preview/1",
            "profile": "stub-profile",
            "profileVersion": "0.0.0",
            "seed": 31,
            "model": {"actors": []}
        })
    );
    let refusing =
        ProcessPreviewAdapter::new(stub("runner-echo-stdin.sh"), stub("validator-refuses.sh"))
            .with_policies(policies())
            .with_runner_args([RUNNER_OUTPUT_SLOT])
            .with_profile("stub-profile", "0.0.0");
    let error = refusing
        .preview(&json!({"actors": []}), 31)
        .expect_err("refusing validator fails the trait preview");
    assert!(error.contains("ValidatorRefused"), "{error}");
}

#[test]
fn simulation_adapter_without_a_profile_reference_fails() {
    let error = real_shaped(&[])
        .preview(&json!({}), 1)
        .expect_err("no profile reference");
    assert!(error.contains("profile"), "{error}");
}

#[test]
#[cfg(unix)]
fn runner_output_with_a_float_is_malformed() {
    let path = scratch("runner-float.sh");
    executable(&path, "#!/bin/sh\nprintf '{\"x\":1.5}\\n' > \"$1\"\n");
    let outcome = ProcessPreviewAdapter::new(path, stub("validator-accepts-any.sh"))
        .with_policies(policies())
        .with_runner_args([RUNNER_OUTPUT_SLOT])
        .run(None)
        .expect("preview runs");
    let error = outcome.runner_output_document().expect_err("float refused");
    assert!(
        matches!(error, PreviewError::MalformedDocument { .. }),
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

/// `ums-profile preview` with the stub pair, the policies and the two
/// emit flags, followed by `extra`.
fn cli_preview(runner: &str, validator: &str, extra: &[&str]) -> (bool, String, String) {
    let runner = stub(runner);
    let validator = stub(validator);
    let policies = policies();
    let mut args = vec![
        "preview",
        "--runner",
        runner.to_str().unwrap(),
        "--validator",
        validator.to_str().unwrap(),
        "--policies",
        policies.to_str().unwrap(),
        "--runner-output-flag",
        "--emit-epistemic",
        "--mind-trace-flag",
        "--emit-mind-trace",
    ];
    args.extend_from_slice(extra);
    cli(&args)
}

#[test]
fn cli_preview_prints_the_canonical_response() {
    let (ok, stdout, stderr) = cli_preview("runner-deterministic.sh", "validator-accepts.sh", &[]);
    assert!(ok, "{stderr}");
    let response: Value = serde_json::from_str(stdout.trim()).expect("JSON response");
    assert_eq!(response["replay"], "identical");
    assert_eq!(
        response["mindTrace"]["document"]["contract"],
        "enaction-mind-trace/1"
    );
}

#[test]
fn cli_preview_passes_hyphenated_runner_arguments() {
    let (ok, _stdout, stderr) = cli_preview(
        "runner-deterministic.sh",
        "validator-accepts.sh",
        &["--runner-arg", "--mode", "--runner-arg", "tamper"],
    );
    assert!(!ok);
    assert!(stderr.contains("ValidatorRefused"), "{stderr}");
    assert!(stderr.contains("digest-matches"), "{stderr}");
    assert!(stderr.contains("package.manifest.digest"), "{stderr}");
}

#[test]
fn cli_preview_refuses_a_nondeterministic_runner() {
    let (ok, _stdout, stderr) = cli_preview(
        "runner-deterministic.sh",
        "validator-accepts.sh",
        &["--runner-arg", "--mode", "--runner-arg", "vary-mind"],
    );
    assert!(!ok);
    assert!(stderr.contains("ReplayMismatch"), "{stderr}");
}

#[test]
fn cli_preview_fails_on_a_missing_binary() {
    let validator = stub("validator-accepts.sh");
    let policies = policies();
    let (ok, _stdout, stderr) = cli(&[
        "preview",
        "--runner",
        "/nonexistent/runner",
        "--validator",
        validator.to_str().unwrap(),
        "--policies",
        policies.to_str().unwrap(),
        "--runner-output-flag",
        "--emit-epistemic",
    ]);
    assert!(!ok);
    assert!(stderr.contains("MissingBinary"), "{stderr}");
}

#[test]
fn cli_preview_clears_the_environment() {
    // cli() sets UMS_PREVIEW_LEAK=1 in the CLI's own environment.
    let runner = stub("runner-env.sh");
    let validator = stub("validator-accepts-any.sh");
    let policies = policies();
    let (ok, stdout, stderr) = cli(&[
        "preview",
        "--runner",
        runner.to_str().unwrap(),
        "--validator",
        validator.to_str().unwrap(),
        "--policies",
        policies.to_str().unwrap(),
        "--runner-arg",
        RUNNER_OUTPUT_SLOT,
    ]);
    assert!(ok, "{stderr}");
    let response: Value = serde_json::from_str(stdout.trim()).expect("JSON response");
    assert_eq!(
        response["runnerOutput"]["document"],
        json!({"env_leaked": 0})
    );
}

#[test]
fn cli_preview_passes_validator_arguments() {
    let (ok, _stdout, stderr) = cli_preview(
        "runner-deterministic.sh",
        "validator-accepts.sh",
        &["--validator-arg", "--expect-profile"],
    );
    assert!(!ok);
    assert!(stderr.contains("ValidatorFailed"), "{stderr}");
    assert!(stderr.contains("--expect-profile"), "{stderr}");
}
