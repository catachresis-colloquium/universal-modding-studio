// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

//! `ProcessPreviewAdapter`: preview is processes over data seams (ADR-0019).
//!
//! The adapter runs a game's pre-built headless runner and then a validator
//! as separate processes. UMS links neither: both are explicit paths to
//! binaries, launched with an argument array (no shell), a cleared
//! environment, a timeout and a stdout size cap.
//!
//! Pipeline:
//!
//! 1. check that both binaries exist and are executable (no fallback);
//! 2. run the runner twice, as two separate processes, with the same input;
//! 3. refuse with [`PreviewError::ReplayMismatch`] unless both stdout byte
//!    strings are identical;
//! 4. pass the runner's stdout to the validator on stdin;
//! 5. validator exit 0 is acceptance; exit 1 is a refusal carrying the
//!    validator's rule name and field path; any other exit is an
//!    operational failure of the validator.
//!
//! There is no stub and no fallback here. The stub binaries used by the
//! protocol tests live under `tests/fixtures/preview`.

use crate::SimulationAdapter;
use crate::canonical::{self, CanonicalError};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Contract tag of the response document (`preview-response.schema.json`).
pub const PREVIEW_RESPONSE_CONTRACT: &str = "ums-enaction-preview-response/1";
/// Contract tag of the request document (`preview-request.schema.json`).
pub const PREVIEW_REQUEST_CONTRACT: &str = "ums-enaction-preview/1";
/// Default per-process timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// Default stdout cap per process (16 MiB).
pub const DEFAULT_STDOUT_CAP: usize = 16 * 1024 * 1024;
/// Stderr kept for diagnostics; the rest is drained and dropped.
const STDERR_KEEP: usize = 8 * 1024;
const POLL: Duration = Duration::from_millis(5);

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
    /// The two runner processes wrote different bytes.
    ReplayMismatch {
        first_len: usize,
        second_len: usize,
        first_difference: usize,
    },
    /// The validator exited 1: the documents are refused.
    ValidatorRefused {
        rule: Option<String>,
        path: Option<String>,
        detail: String,
    },
    /// The validator exited with any status other than 0 or 1.
    ValidatorFailed { status: String, stderr: String },
    /// The runner's stdout is not a sequence of integer-only JSON documents.
    MalformedRunnerOutput { detail: String },
    /// A request could not be built (for example, no profile reference).
    BadRequest { detail: String },
}

impl PreviewError {
    /// Stable name of the failure, printed first in every diagnostic.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::MissingBinary { .. } => "MissingBinary",
            Self::NotExecutable { .. } => "NotExecutable",
            Self::NotExplicitPath { .. } => "NotExplicitPath",
            Self::Spawn { .. } => "Spawn",
            Self::Timeout { .. } => "Timeout",
            Self::OutputTooLarge { .. } => "OutputTooLarge",
            Self::RunnerFailed { .. } => "RunnerFailed",
            Self::ReplayMismatch { .. } => "ReplayMismatch",
            Self::ValidatorRefused { .. } => "ValidatorRefused",
            Self::ValidatorFailed { .. } => "ValidatorFailed",
            Self::MalformedRunnerOutput { .. } => "MalformedRunnerOutput",
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
            Self::ReplayMismatch {
                first_len,
                second_len,
                first_difference,
            } => write!(
                f,
                "two runner processes wrote different bytes ({first_len} and {second_len} bytes; first difference at byte {first_difference})"
            ),
            Self::ValidatorRefused { rule, path, detail } => write!(
                f,
                "validator refused the documents: rule {} at {}{}",
                rule.as_deref().unwrap_or("<unnamed>"),
                path.as_deref().unwrap_or("<no path>"),
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", detail.trim_end())
                }
            ),
            Self::ValidatorFailed { status, stderr } => write!(
                f,
                "validator could not run ({status}): {}",
                stderr.trim_end()
            ),
            Self::MalformedRunnerOutput { detail } => f.write_str(detail),
            Self::BadRequest { detail } => f.write_str(detail),
        }
    }
}

impl std::error::Error for PreviewError {}

/// The captured result of one accepted preview.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewOutcome {
    /// Runner stdout; identical across both runs.
    pub runner_stdout: Vec<u8>,
    /// Validator stdout (exit 0).
    pub validator_stdout: Vec<u8>,
}

impl PreviewOutcome {
    /// Parse runner stdout as newline-separated JSON documents, each
    /// integer-only within +-(2^53-1). Blank lines are ignored.
    pub fn documents(&self) -> Result<Vec<Value>, PreviewError> {
        parse_documents(&self.runner_stdout)
    }

    /// The response document described by
    /// `adapters/enaction/preview-response.schema.json`.
    pub fn response(&self) -> Result<Value, PreviewError> {
        Ok(json!({
            "adapterContract": PREVIEW_RESPONSE_CONTRACT,
            "replay": "identical",
            "runnerStdoutBytes": self.runner_stdout.len(),
            "validation": {
                "exit": 0,
                "stdout": String::from_utf8_lossy(&self.validator_stdout),
            },
            "documents": self.documents()?,
        }))
    }
}

/// Parse a stream of newline-separated JSON documents.
pub fn parse_documents(bytes: &[u8]) -> Result<Vec<Value>, PreviewError> {
    let text = std::str::from_utf8(bytes).map_err(|error| PreviewError::MalformedRunnerOutput {
        detail: format!("runner stdout is not UTF-8: {error}"),
    })?;
    let mut documents = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(line).map_err(|error| PreviewError::MalformedRunnerOutput {
                detail: format!("line {}: not JSON: {error}", index + 1),
            })?;
        canonical::check_integers(&value).map_err(|error: CanonicalError| {
            PreviewError::MalformedRunnerOutput {
                detail: format!("line {}: {error}", index + 1),
            }
        })?;
        documents.push(value);
    }
    if documents.is_empty() {
        return Err(PreviewError::MalformedRunnerOutput {
            detail: "runner wrote no document".into(),
        });
    }
    Ok(documents)
}

#[derive(Clone, Debug)]
pub struct ProcessPreviewAdapter {
    runner: PathBuf,
    validator: PathBuf,
    runner_args: Vec<OsString>,
    validator_args: Vec<OsString>,
    timeout: Duration,
    stdout_cap: usize,
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
            timeout: DEFAULT_TIMEOUT,
            stdout_cap: DEFAULT_STDOUT_CAP,
            profile: None,
        }
    }

    pub fn with_runner_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.runner_args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_validator_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.validator_args = args.into_iter().map(Into::into).collect();
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
        let runner = explicit_executable(Role::Runner, &self.runner)?;
        let validator = explicit_executable(Role::Validator, &self.validator)?;

        let mut runs = Vec::with_capacity(2);
        for run in 1..=2u8 {
            let captured = run_process(
                Role::Runner,
                &runner,
                &self.runner_args,
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
            runs.push(captured.stdout);
        }
        let second = runs.pop().unwrap_or_default();
        let first = runs.pop().unwrap_or_default();
        if first != second {
            let first_difference = first
                .iter()
                .zip(&second)
                .position(|(a, b)| a != b)
                .unwrap_or(first.len().min(second.len()));
            return Err(PreviewError::ReplayMismatch {
                first_len: first.len(),
                second_len: second.len(),
                first_difference,
            });
        }

        let checked = run_process(
            Role::Validator,
            &validator,
            &self.validator_args,
            Some(&first),
            self.timeout,
            self.stdout_cap,
        )?;
        match checked.status.code() {
            Some(0) => Ok(PreviewOutcome {
                runner_stdout: first,
                validator_stdout: checked.stdout,
            }),
            Some(1) => {
                let (rule, path) = parse_fault(&checked.stdout)
                    .or_else(|| parse_fault(&checked.stderr))
                    .unwrap_or((None, None));
                let detail = if rule.is_some() {
                    String::new()
                } else {
                    excerpt(&checked.stdout, &checked.stderr)
                };
                Err(PreviewError::ValidatorRefused { rule, path, detail })
            }
            _ => Err(PreviewError::ValidatorFailed {
                status: checked.status.to_string(),
                stderr: excerpt(&checked.stderr, &checked.stdout),
            }),
        }
    }
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

/// Read a validator fault. Accepted forms (one JSON object, either as the
/// whole stream or on one line): `{"rule": R, "path": P}`, or
/// `{"faults": [{"rule": R, "path": P}, ...]}` (the first fault is used).
fn parse_fault(bytes: &[u8]) -> Option<(Option<String>, Option<String>)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let candidates = std::iter::once(text.trim()).chain(text.lines().map(str::trim));
    for candidate in candidates {
        let Ok(value) = serde_json::from_str::<Value>(candidate) else {
            continue;
        };
        let fault = match value.get("faults").and_then(Value::as_array) {
            Some(faults) => faults.first().cloned().unwrap_or(Value::Null),
            None => value,
        };
        let rule = fault.get("rule").and_then(Value::as_str).map(str::to_owned);
        if rule.is_some() {
            let path = fault.get("path").and_then(Value::as_str).map(str::to_owned);
            return Some((rule, path));
        }
    }
    None
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
    use super::parse_fault;

    #[test]
    fn fault_forms_are_read() {
        assert_eq!(
            parse_fault(br#"{"rule":"BadRanking","path":"/ticks/3"}"#),
            Some((Some("BadRanking".into()), Some("/ticks/3".into())))
        );
        assert_eq!(
            parse_fault(b"note\n{\"faults\":[{\"rule\":\"DanglingCause\",\"path\":\"/ticks/0/affect/caused_by/0\"}]}\n"),
            Some((
                Some("DanglingCause".into()),
                Some("/ticks/0/affect/caused_by/0".into())
            ))
        );
        assert_eq!(parse_fault(b"plain text"), None);
    }
}
