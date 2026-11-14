// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! `ProcessPreviewAdapter`: preview is processes over data seams (ADR-0019).
//!
//! The adapter runs a game's pre-built headless runner and then Enaction's
//! validator as separate processes. UMS links neither: both are explicit
//! paths to binaries, launched with an argument array (no shell), a cleared
//! environment, a timeout and a stdout size cap.
//!
//! Documents cross as files, one document per file, as
//! `idaptik-weevil-replay` writes them (`--emit-epistemic PATH
//! [--emit-mind-trace PATH]`). The caller gives the runner's argument
//! template; the whole arguments [`RUNNER_OUTPUT_SLOT`] and
//! [`MIND_TRACE_SLOT`] are replaced with paths that the adapter chooses.
//!
//! Pipeline:
//!
//! 1. check that both binaries exist and are executable, and that the
//!    caller's policies file exists (no fallback);
//! 2. create one private temporary directory (mode 0700 on Unix) with a
//!    subdirectory per run, removed when the preview ends;
//! 3. run the runner twice, as two separate processes, each writing into its
//!    own subdirectory;
//! 4. read each document with a size cap and refuse with
//!    [`PreviewError::ReplayMismatch`] unless each pair is byte-identical.
//!    The runners' stdout is not compared: a runner may print the paths it
//!    wrote, which differ between the runs by construction;
//! 5. run `validator --policies FILE [--mind-trace FILE] [extra...]` with
//!    the runner output on stdin (CAC-KERNEL section 9). Exit 0 is
//!    acceptance; exit 1 is a refusal, read from the one canonical line
//!    `{"faults":[{"detail","path","rule"}]}` on stdout; any other exit is
//!    an operational failure of the validator.
//!
//! The mind trace is optional: a template without [`MIND_TRACE_SLOT`]
//! previews the runner output alone.
//!
//! There is no stub and no fallback here. The stub binaries used by the
//! protocol tests live under `tests/fixtures/preview`.

use crate::SimulationAdapter;
use crate::canonical::{self, CanonicalError};
use serde_json::{Map, Value, json};
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Contract tag of the response document (`preview-response.schema.json`).
pub const PREVIEW_RESPONSE_CONTRACT: &str = "ums-enaction-preview-response/2";
/// Contract tag of the request document (`preview-request.schema.json`).
pub const PREVIEW_REQUEST_CONTRACT: &str = "ums-enaction-preview/1";
/// Runner argument replaced by the path of the runner-output file.
pub const RUNNER_OUTPUT_SLOT: &str = "{runner_output}";
/// Runner argument replaced by the path of the mind-trace file.
pub const MIND_TRACE_SLOT: &str = "{mind_trace}";
/// Default per-process timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// Default stdout cap per process (16 MiB).
pub const DEFAULT_STDOUT_CAP: usize = 16 * 1024 * 1024;
/// Default size cap per document file (64 MiB).
pub const DEFAULT_DOCUMENT_CAP: usize = 64 * 1024 * 1024;
/// Stderr kept for diagnostics; the rest is drained and dropped.
const STDERR_KEEP: usize = 8 * 1024;
const POLL: Duration = Duration::from_millis(5);

const RUNNER_OUTPUT: &str = "runner-output";
const MIND_TRACE: &str = "mind-trace";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Runner,
    Validator,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Runner => "runner",
            Self::Validator => "validator",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreviewError {
    /// The path names no file.
    MissingBinary { role: Role, path: PathBuf },
    /// The file exists but is not an executable regular file.
    NotExecutable { role: Role, path: PathBuf },
    /// A bare command name, which would be resolved through `PATH`.
    NotExplicitPath { role: Role, path: PathBuf },
    /// The caller's policies file does not exist or is not a file.
    MissingPolicies { path: PathBuf },
    /// The operating system refused to start the process.
    Spawn { role: Role, detail: String },
    /// The process did not finish within the timeout and was killed.
    Timeout { role: Role, limit: Duration },
    /// The process wrote more than the stdout cap and was killed.
    OutputTooLarge { role: Role, cap: usize },
    /// The runner exited unsuccessfully (`run` is 1 or 2).
    RunnerFailed {
        run: u8,
        status: String,
        stderr: String,
    },
    /// The runner exited 0 without writing a document it was asked for.
    MissingDocument { run: u8, document: &'static str },
    /// A document file is larger than the document cap.
    DocumentTooLarge {
        run: u8,
        document: &'static str,
        cap: usize,
    },
    /// The two runner processes wrote different bytes into one document.
    ReplayMismatch {
        document: &'static str,
        first_len: usize,
        second_len: usize,
        first_difference: usize,
    },
    /// The validator exited 1: the documents are refused. `rule`, `path`
    /// and `detail` are those of the first fault; `more` counts the rest.
    ValidatorRefused {
        rule: Option<String>,
        path: Option<String>,
        detail: String,
        more: usize,
    },
    /// The validator exited with any status other than 0 or 1.
    ValidatorFailed { status: String, stderr: String },
    /// A document is not one integer-only JSON value.
    MalformedDocument {
        document: &'static str,
        detail: String,
    },
    /// A request could not be built (no profile reference, no policies, an
    /// ambiguous runner template, or an unusable temporary directory).
    BadRequest { detail: String },
}

impl PreviewError {
    /// Stable name of the failure, printed first in every diagnostic.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::MissingBinary { .. } => "MissingBinary",
            Self::NotExecutable { .. } => "NotExecutable",
            Self::NotExplicitPath { .. } => "NotExplicitPath",
            Self::MissingPolicies { .. } => "MissingPolicies",
            Self::Spawn { .. } => "Spawn",
            Self::Timeout { .. } => "Timeout",
            Self::OutputTooLarge { .. } => "OutputTooLarge",
            Self::RunnerFailed { .. } => "RunnerFailed",
            Self::MissingDocument { .. } => "MissingDocument",
            Self::DocumentTooLarge { .. } => "DocumentTooLarge",
            Self::ReplayMismatch { .. } => "ReplayMismatch",
            Self::ValidatorRefused { .. } => "ValidatorRefused",
            Self::ValidatorFailed { .. } => "ValidatorFailed",
            Self::MalformedDocument { .. } => "MalformedDocument",
            Self::BadRequest { .. } => "BadRequest",
        }
    }
}

impl fmt::Display for PreviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.kind())?;
        match self {
            Self::MissingBinary { role, path } => {
                write!(f, "{role} binary not found at {}", path.display())
            }
            Self::NotExecutable { role, path } => write!(
                f,
                "{role} at {} is not an executable regular file",
                path.display()
            ),
            Self::NotExplicitPath { role, path } => write!(
                f,
                "{role} '{}' is a bare name; give an explicit path (no PATH search)",
                path.display()
            ),
            Self::MissingPolicies { path } => {
                write!(f, "policies file not found at {}", path.display())
            }
            Self::Spawn { role, detail } => write!(f, "could not start {role}: {detail}"),
            Self::Timeout { role, limit } => {
                write!(f, "{role} did not finish within {} ms", limit.as_millis())
            }
            Self::OutputTooLarge { role, cap } => {
                write!(f, "{role} wrote more than {cap} bytes to stdout")
            }
            Self::RunnerFailed {
                run,
                status,
                stderr,
            } => write!(
                f,
                "runner run {run} failed ({status}): {}",
                stderr.trim_end()
            ),
            Self::MissingDocument { run, document } => {
                write!(f, "runner run {run} exited 0 but wrote no {document} file")
            }
            Self::DocumentTooLarge { run, document, cap } => write!(
                f,
                "runner run {run} wrote a {document} file larger than {cap} bytes"
            ),
            Self::ReplayMismatch {
                document,
                first_len,
                second_len,
                first_difference,
            } => write!(
                f,
                "two runner processes wrote different {document} bytes ({first_len} and {second_len} bytes; first difference at byte {first_difference})"
            ),
            Self::ValidatorRefused {
                rule,
                path,
                detail,
                more,
            } => {
                write!(
                    f,
                    "validator refused the documents: rule {} at {}",
                    rule.as_deref().unwrap_or("<unnamed>"),
                    path.as_deref().unwrap_or("<no path>"),
                )?;
                if !detail.is_empty() {
                    write!(f, " ({})", detail.trim_end())?;
                }
                if *more > 0 {
                    write!(
                        f,
                        "; and {more} more fault{}",
                        if *more == 1 { "" } else { "s" }
                    )?;
                }
                Ok(())
            }
            Self::ValidatorFailed { status, stderr } => write!(
                f,
                "validator could not run ({status}): {}",
                stderr.trim_end()
            ),
            Self::MalformedDocument { document, detail } => write!(f, "{document}: {detail}"),
            Self::BadRequest { detail } => f.write_str(detail),
        }
    }
}

impl std::error::Error for PreviewError {}

/// The captured result of one accepted preview.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewOutcome {
    /// The runner-output file; identical across both runs.
    pub runner_output: Vec<u8>,
    /// The mind-trace file, when the template asked for one; identical
    /// across both runs.
    pub mind_trace: Option<Vec<u8>>,
    /// Stdout of the first runner process. Not compared between runs.
    pub runner_stdout: Vec<u8>,
    /// Validator stdout (exit 0).
    pub validator_stdout: Vec<u8>,
}

impl PreviewOutcome {
    /// The runner output as one integer-only JSON value.
    pub fn runner_output_document(&self) -> Result<Value, PreviewError> {
        parse_document(RUNNER_OUTPUT, &self.runner_output)
    }

    /// The mind trace as one integer-only JSON value, if there is one.
    pub fn mind_trace_document(&self) -> Result<Option<Value>, PreviewError> {
        self.mind_trace
            .as_deref()
            .map(|bytes| parse_document(MIND_TRACE, bytes))
            .transpose()
    }

    /// The response document described by
    /// `adapters/enaction/preview-response.schema.json`.
    pub fn response(&self) -> Result<Value, PreviewError> {
        let mut response = Map::new();
        response.insert("adapterContract".into(), json!(PREVIEW_RESPONSE_CONTRACT));
        response.insert("replay".into(), json!("identical"));
        response.insert(
            "runnerOutput".into(),
            json!({
                "bytes": self.runner_output.len(),
                "document": self.runner_output_document()?,
            }),
        );
        if let (Some(bytes), Some(document)) = (&self.mind_trace, self.mind_trace_document()?) {
            response.insert(
                "mindTrace".into(),
                json!({"bytes": bytes.len(), "document": document}),
            );
        }
        response.insert(
            "validation".into(),
            json!({
                "exit": 0,
                "stdout": String::from_utf8_lossy(&self.validator_stdout),
            }),
        );
        Ok(Value::Object(response))
    }
}

/// Parse one document file: UTF-8, exactly one JSON value (surrounding
/// whitespace allowed), every number an integer within +-(2^53-1).
pub fn parse_document(document: &'static str, bytes: &[u8]) -> Result<Value, PreviewError> {
    let malformed = |detail: String| PreviewError::MalformedDocument { document, detail };
    let text =
        std::str::from_utf8(bytes).map_err(|error| malformed(format!("not UTF-8: {error}")))?;
    let value: Value =
        serde_json::from_str(text).map_err(|error| malformed(format!("not JSON: {error}")))?;
    canonical::check_integers(&value)
        .map_err(|error: CanonicalError| malformed(error.to_string()))?;
    Ok(value)
}

/// A private temporary directory, removed with its contents on drop.
#[derive(Debug)]
pub struct PrivateDir {
    path: PathBuf,
}

impl PrivateDir {
    /// Create a fresh directory under the system temporary directory. It is
    /// created with `create_dir` (never reused) and, on Unix, mode 0700.
    pub fn create() -> io::Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        for _ in 0..16 {
            let count = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = base.join(format!(
                "ums-preview-{}-{nanos}-{count}",
                std::process::id()
            ));
            match create_private_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "no fresh temporary directory name",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(unix)]
fn create_private_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir(path)
}

/// Which documents a runner template asks for.
struct Template {
    mind_trace: bool,
}

fn check_template(args: &[OsString]) -> Result<Template, PreviewError> {
    let bad = |detail: String| PreviewError::BadRequest { detail };
    let mut runner_output = 0;
    let mut mind_trace = 0;
    for arg in args {
        let text = arg.to_string_lossy();
        if text == RUNNER_OUTPUT_SLOT {
            runner_output += 1;
        } else if text == MIND_TRACE_SLOT {
            mind_trace += 1;
        } else if text.contains(RUNNER_OUTPUT_SLOT) || text.contains(MIND_TRACE_SLOT) {
            return Err(bad(format!(
                "runner argument '{text}' contains a slot inside a larger argument; a slot must be a whole argument"
            )));
        }
    }
    if runner_output != 1 {
        return Err(bad(format!(
            "the runner template must contain {RUNNER_OUTPUT_SLOT} exactly once as a whole argument (found {runner_output})"
        )));
    }
    if mind_trace > 1 {
        return Err(bad(format!(
            "the runner template may contain {MIND_TRACE_SLOT} at most once (found {mind_trace})"
        )));
    }
    Ok(Template {
        mind_trace: mind_trace == 1,
    })
}

#[derive(Clone, Debug)]
pub struct ProcessPreviewAdapter {
    runner: PathBuf,
    validator: PathBuf,
    runner_args: Vec<OsString>,
    validator_args: Vec<OsString>,
    policies: Option<PathBuf>,
    timeout: Duration,
    stdout_cap: usize,
    document_cap: usize,
    profile: Option<(String, String)>,
}

impl ProcessPreviewAdapter {
    /// An adapter for one runner binary and one validator binary. Both are
    /// explicit paths; nothing is searched for and nothing is substituted.
    pub fn new(runner: impl Into<PathBuf>, validator: impl Into<PathBuf>) -> Self {
        Self {
            runner: runner.into(),
            validator: validator.into(),
            runner_args: Vec::new(),
            validator_args: Vec::new(),
            policies: None,
            timeout: DEFAULT_TIMEOUT,
            stdout_cap: DEFAULT_STDOUT_CAP,
            document_cap: DEFAULT_DOCUMENT_CAP,
            profile: None,
        }
    }

    /// The runner's argument template. It must contain
    /// [`RUNNER_OUTPUT_SLOT`] once and may contain [`MIND_TRACE_SLOT`] once,
    /// each as a whole argument.
    pub fn with_runner_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.runner_args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Extra validator arguments, passed after `--policies` and
    /// `--mind-trace`.
    pub fn with_validator_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.validator_args = args.into_iter().map(Into::into).collect();
        self
    }

    /// The caller's `enaction-policies/1` file, passed to the validator as
    /// `--policies`. Required: the policies are the caller's, never the
    /// runner's (CAC-KERNEL section 9).
    pub fn with_policies(mut self, path: impl Into<PathBuf>) -> Self {
        self.policies = Some(path.into());
        self
    }

    /// Per-process timeout, applied to each of the three processes.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Maximum stdout bytes accepted from each process.
    pub fn with_stdout_cap(mut self, cap: usize) -> Self {
        self.stdout_cap = cap;
        self
    }

    /// Maximum size of each document file.
    pub fn with_document_cap(mut self, cap: usize) -> Self {
        self.document_cap = cap;
        self
    }

    /// Profile id and version written into the preview request that
    /// [`SimulationAdapter::preview`] sends to the runner.
    pub fn with_profile(mut self, id: impl Into<String>, version: impl Into<String>) -> Self {
        self.profile = Some((id.into(), version.into()));
        self
    }

    /// The `ums-enaction-preview/1` request for a model and seed.
    pub fn preview_request(&self, model: &Value, seed: u64) -> Result<Value, PreviewError> {
        let (id, version) = self
            .profile
            .as_ref()
            .ok_or_else(|| PreviewError::BadRequest {
                detail: "no profile reference: call with_profile(id, version)".into(),
            })?;
        Ok(json!({
            "adapterContract": PREVIEW_REQUEST_CONTRACT,
            "profile": id,
            "profileVersion": version,
            "seed": seed,
            "model": model,
        }))
    }

    /// Run the full pipeline. `stdin`, if given, is written to both runner
    /// processes; otherwise their stdin is empty.
    pub fn run(&self, stdin: Option<&[u8]>) -> Result<PreviewOutcome, PreviewError> {
        let template = check_template(&self.runner_args)?;
        let policies = self.policies.as_ref().ok_or_else(|| PreviewError::BadRequest {
            detail: "no policies file: the validator's --policies is the caller's (call with_policies)".into(),
        })?;
        if !policies.is_file() {
            return Err(PreviewError::MissingPolicies {
                path: policies.clone(),
            });
        }
        let policies =
            std::fs::canonicalize(policies).map_err(|_| PreviewError::MissingPolicies {
                path: policies.clone(),
            })?;
        let runner = explicit_executable(Role::Runner, &self.runner)?;
        let validator = explicit_executable(Role::Validator, &self.validator)?;

        let scratch = PrivateDir::create().map_err(|error| PreviewError::BadRequest {
            detail: format!("cannot create a private temporary directory: {error}"),
        })?;
        let mut runs = Vec::with_capacity(2);
        for run in 1..=2u8 {
            runs.push(self.run_runner(&runner, scratch.path(), run, &template, stdin)?);
        }
        let second = runs.pop().unwrap_or_default();
        let first = runs.pop().unwrap_or_default();
        compare(RUNNER_OUTPUT, &first.runner_output, &second.runner_output)?;
        if let (Some(a), Some(b)) = (&first.mind_trace, &second.mind_trace) {
            compare(MIND_TRACE, a, b)?;
        }

        let mut args: Vec<OsString> = vec!["--policies".into(), policies.into_os_string()];
        if template.mind_trace {
            args.push("--mind-trace".into());
            args.push(first.mind_trace_path.clone().into_os_string());
        }
        args.extend(self.validator_args.iter().cloned());
        let checked = run_process(
            Role::Validator,
            &validator,
            &args,
            Some(&first.runner_output),
            self.timeout,
            self.stdout_cap,
        )?;
        drop(scratch);
        match checked.status.code() {
            Some(0) => Ok(PreviewOutcome {
                runner_output: first.runner_output,
                mind_trace: first.mind_trace,
                runner_stdout: first.stdout,
                validator_stdout: checked.stdout,
            }),
            Some(1) => Err(match parse_faults(&checked.stdout) {
                Some(faults) => {
                    let mut faults = faults.into_iter();
                    let (rule, path, detail) = faults.next().unwrap_or_default();
                    PreviewError::ValidatorRefused {
                        rule,
                        path,
                        detail: detail.unwrap_or_default(),
                        more: faults.count(),
                    }
                }
                None => PreviewError::ValidatorRefused {
                    rule: None,
                    path: None,
                    detail: excerpt(&checked.stdout, &checked.stderr),
                    more: 0,
                },
            }),
            _ => Err(PreviewError::ValidatorFailed {
                status: checked.status.to_string(),
                stderr: excerpt(&checked.stderr, &checked.stdout),
            }),
        }
    }

    fn run_runner(
        &self,
        runner: &Path,
        scratch: &Path,
        run: u8,
        template: &Template,
        stdin: Option<&[u8]>,
    ) -> Result<RunnerRun, PreviewError> {
        let dir = scratch.join(format!("run-{run}"));
        create_private_dir(&dir).map_err(|error| PreviewError::BadRequest {
            detail: format!("cannot create {}: {error}", dir.display()),
        })?;
        let runner_output_path = dir.join("runner-output.json");
        let mind_trace_path = dir.join("mind-trace.json");
        let args: Vec<OsString> = self
            .runner_args
            .iter()
            .map(|arg| {
                if arg == RUNNER_OUTPUT_SLOT {
                    runner_output_path.clone().into_os_string()
                } else if arg == MIND_TRACE_SLOT {
                    mind_trace_path.clone().into_os_string()
                } else {
                    arg.clone()
                }
            })
            .collect();
        let captured = run_process(
            Role::Runner,
            runner,
            &args,
            stdin,
            self.timeout,
            self.stdout_cap,
        )?;
        if !captured.status.success() {
            return Err(PreviewError::RunnerFailed {
                run,
                status: captured.status.to_string(),
                stderr: String::from_utf8_lossy(&captured.stderr).into_owned(),
            });
        }
        let runner_output =
            read_document(run, RUNNER_OUTPUT, &runner_output_path, self.document_cap)?;
        let mind_trace = if template.mind_trace {
            Some(read_document(
                run,
                MIND_TRACE,
                &mind_trace_path,
                self.document_cap,
            )?)
        } else {
            None
        };
        Ok(RunnerRun {
            runner_output,
            mind_trace,
            mind_trace_path,
            stdout: captured.stdout,
        })
    }
}

#[derive(Default)]
struct RunnerRun {
    runner_output: Vec<u8>,
    mind_trace: Option<Vec<u8>>,
    mind_trace_path: PathBuf,
    stdout: Vec<u8>,
}

fn read_document(
    run: u8,
    document: &'static str,
    path: &Path,
    cap: usize,
) -> Result<Vec<u8>, PreviewError> {
    let file =
        std::fs::File::open(path).map_err(|_| PreviewError::MissingDocument { run, document })?;
    let too_large = PreviewError::DocumentTooLarge { run, document, cap };
    let length = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    if length > cap as u64 {
        return Err(too_large);
    }
    let mut bytes = Vec::new();
    file.take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PreviewError::MissingDocument { run, document })?;
    if bytes.len() > cap {
        return Err(too_large);
    }
    Ok(bytes)
}

fn compare(document: &'static str, first: &[u8], second: &[u8]) -> Result<(), PreviewError> {
    if first == second {
        return Ok(());
    }
    let first_difference = first
        .iter()
        .zip(second)
        .position(|(a, b)| a != b)
        .unwrap_or(first.len().min(second.len()));
    Err(PreviewError::ReplayMismatch {
        document,
        first_len: first.len(),
        second_len: second.len(),
        first_difference,
    })
}

impl SimulationAdapter for ProcessPreviewAdapter {
    fn preview(&self, model: &Value, seed: u64) -> Result<Value, String> {
        let request = self
            .preview_request(model, seed)
            .map_err(|error| error.to_string())?;
        let bytes = canonical::to_canonical_bytes(&request).map_err(|error| {
            PreviewError::BadRequest {
                detail: error.to_string(),
            }
            .to_string()
        })?;
        self.run(Some(&bytes))
            .and_then(|outcome| outcome.response())
            .map_err(|error| error.to_string())
    }
}

type Fault = (Option<String>, Option<String>, Option<String>);

/// Read the validator's refusal: stdout is exactly one line, the canonical
/// object `{"faults":[{"detail","path","rule"}, ...]}` (CAC-KERNEL section
/// 9). Stderr is for a human reader and is not parsed. Anything else is
/// `None`.
fn parse_faults(stdout: &[u8]) -> Option<Vec<Fault>> {
    let text = std::str::from_utf8(stdout).ok()?;
    let line = text.strip_suffix('\n').unwrap_or(text);
    if line.contains('\n') {
        return None;
    }
    let value: Value = serde_json::from_str(line).ok()?;
    let faults = value.get("faults")?.as_array()?;
    if faults.is_empty() {
        return None;
    }
    let field =
        |fault: &Value, name: &str| fault.get(name).and_then(Value::as_str).map(str::to_owned);
    Some(
        faults
            .iter()
            .map(|fault| {
                (
                    field(fault, "rule"),
                    field(fault, "path"),
                    field(fault, "detail"),
                )
            })
            .collect(),
    )
}

fn excerpt(primary: &[u8], secondary: &[u8]) -> String {
    let chosen = if primary.iter().any(|b| !b.is_ascii_whitespace()) {
        primary
    } else {
        secondary
    };
    let text = String::from_utf8_lossy(chosen);
    text.chars().take(2048).collect()
}

fn explicit_executable(role: Role, path: &Path) -> Result<PathBuf, PreviewError> {
    let bare = path
        .parent()
        .is_none_or(|parent| parent.as_os_str().is_empty());
    if bare && !path.is_absolute() {
        return Err(PreviewError::NotExplicitPath {
            role,
            path: path.to_owned(),
        });
    }
    let metadata = std::fs::metadata(path).map_err(|_| PreviewError::MissingBinary {
        role,
        path: path.to_owned(),
    })?;
    if !metadata.is_file() || !is_executable(&metadata) {
        return Err(PreviewError::NotExecutable {
            role,
            path: path.to_owned(),
        });
    }
    std::fs::canonicalize(path).map_err(|_| PreviewError::MissingBinary {
        role,
        path: path.to_owned(),
    })
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    true
}

struct Captured {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Read a pipe to the end, keeping at most `keep` bytes. Bytes beyond that
/// are drained (so the child never blocks on a full pipe) and `overflow` is
/// raised.
fn capped_reader<R: Read + Send + 'static>(
    mut pipe: R,
    keep: usize,
    overflow: Option<Arc<AtomicBool>>,
) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    let room = keep.saturating_sub(kept.len());
                    kept.extend_from_slice(&chunk[..read.min(room)]);
                    if read > room
                        && let Some(flag) = &overflow
                    {
                        flag.store(true, Ordering::SeqCst);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        kept
    })
}

fn run_process(
    role: Role,
    program: &Path,
    args: &[OsString],
    stdin: Option<&[u8]>,
    timeout: Duration,
    stdout_cap: usize,
) -> Result<Captured, PreviewError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .env_clear()
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| PreviewError::Spawn {
        role,
        detail: error.to_string(),
    })?;

    let writer = match (stdin, child.stdin.take()) {
        (Some(bytes), Some(mut pipe)) => {
            let bytes = bytes.to_vec();
            // A child that exits without reading its input closes the pipe;
            // the write error is then irrelevant, its exit status decides.
            Some(thread::spawn(move || {
                let _ = pipe.write_all(&bytes);
            }))
        }
        _ => None,
    };
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child
        .stdout
        .take()
        .map(|pipe| capped_reader(pipe, stdout_cap, Some(Arc::clone(&overflow))));
    let stderr = child
        .stderr
        .take()
        .map(|pipe| capped_reader(pipe, STDERR_KEEP, None));

    let deadline = Instant::now() + timeout;
    let status = loop {
        if overflow.load(Ordering::SeqCst) {
            kill(&mut child);
            return Err(PreviewError::OutputTooLarge {
                role,
                cap: stdout_cap,
            });
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                kill(&mut child);
                return Err(PreviewError::Timeout {
                    role,
                    limit: timeout,
                });
            }
            Ok(None) => thread::sleep(POLL),
            Err(error) => {
                kill(&mut child);
                return Err(PreviewError::Spawn {
                    role,
                    detail: error.to_string(),
                });
            }
        }
    };
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    let stdout = stdout
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    if overflow.load(Ordering::SeqCst) {
        return Err(PreviewError::OutputTooLarge {
            role,
            cap: stdout_cap,
        });
    }
    Ok(Captured {
        status,
        stdout,
        stderr,
    })
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::parse_faults;

    #[test]
    fn the_canonical_fault_line_is_read() {
        let line = b"{\"faults\":[{\"detail\":\"stale\",\"path\":\"package.manifest.digest\",\"rule\":\"digest-matches\"},{\"detail\":\"x\",\"path\":\"mind_trace.ticks[0]\",\"rule\":\"BadRanking\"}]}\n";
        let faults = parse_faults(line).expect("faults");
        assert_eq!(faults.len(), 2);
        assert_eq!(faults[0].0.as_deref(), Some("digest-matches"));
        assert_eq!(faults[0].1.as_deref(), Some("package.manifest.digest"));
        assert_eq!(faults[0].2.as_deref(), Some("stale"));
    }

    #[test]
    fn anything_but_one_fault_line_is_not_read() {
        // A4 also read a bare {"rule","path"} object and scanned stderr; the
        // real validator prints neither, so neither is accepted now.
        assert_eq!(
            parse_faults(b"{\"rule\":\"BadRanking\",\"path\":\"x\"}\n"),
            None
        );
        assert_eq!(
            parse_faults(b"note\n{\"faults\":[{\"rule\":\"R\"}]}\n"),
            None
        );
        assert_eq!(parse_faults(b"{\"faults\":[]}\n"), None);
        assert_eq!(parse_faults(b"plain text"), None);
    }
}
