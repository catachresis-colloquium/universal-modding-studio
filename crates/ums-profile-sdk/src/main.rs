// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2025-2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use ums_profile_sdk::ProfileRegistry;
use ums_profile_sdk::preview::{MIND_TRACE_SLOT, ProcessPreviewAdapter, RUNNER_OUTPUT_SLOT};

#[derive(Parser)]
#[command(
    name = "ums-profile",
    version,
    about = "Validate and compile isolated UMS game profiles"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile any registered executable profile against its game's contract.
    Compile {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        game_root: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    CompileIdaptik {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        idaptik_root: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Validate {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        fixture: PathBuf,
    },
    /// Run a pre-built headless runner twice and a validator once, as
    /// separate processes, and print the replay-checked response (ADR-0019).
    ///
    /// Each document crosses as its own file. The runner's argument
    /// template may name the files with the whole arguments
    /// `{runner_output}` and `{mind_trace}`; `--runner-output-flag F` and
    /// `--mind-trace-flag F` append `F {runner_output}` and
    /// `F {mind_trace}` for a runner such as `idaptik-weevil-replay`.
    Preview {
        /// Explicit path to the runner binary.
        #[arg(long)]
        runner: PathBuf,
        /// Explicit path to the validator binary (`enaction-validate`).
        #[arg(long)]
        validator: PathBuf,
        /// The caller's `enaction-policies/1` file, passed as `--policies`.
        #[arg(long)]
        policies: PathBuf,
        /// One runner argument; repeat for more. Passed as an argument array.
        #[arg(long = "runner-arg", allow_hyphen_values = true)]
        runner_args: Vec<String>,
        /// Runner flag that takes the runner-output path, for example
        /// `--emit-epistemic`.
        #[arg(long, allow_hyphen_values = true)]
        runner_output_flag: Option<String>,
        /// Runner flag that takes the mind-trace path, for example
        /// `--emit-mind-trace`. Without it the preview has no mind trace.
        #[arg(long, allow_hyphen_values = true)]
        mind_trace_flag: Option<String>,
        /// One extra validator argument; repeat for more.
        #[arg(long = "validator-arg", allow_hyphen_values = true)]
        validator_args: Vec<String>,
        /// Per-process timeout in milliseconds.
        #[arg(long, default_value_t = 60_000)]
        timeout_ms: u64,
        /// Print the designer report instead of the response document.
        #[arg(long)]
        explain: bool,
    },
}

fn run(command: Command) -> Result<String, String> {
    match command {
        Command::Compile {
            profile,
            source,
            game_root,
            output,
        } => {
            let registry = ProfileRegistry::with_builtins();
            let compiler = registry
                .package_compiler(&profile)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("{profile} does not declare a package compiler"))?;
            let package = compiler
                .compile(&source, &game_root)
                .map_err(|e| e.to_string())?;
            ums_profiles::write_pretty_json(&output, &package).map_err(|e| e.to_string())?;
            Ok(format!("compiled {}", output.display()))
        }
        Command::CompileIdaptik {
            source,
            idaptik_root,
            output,
        } => {
            let registry = ProfileRegistry::with_builtins();
            let compiler = registry
                .package_compiler("idaptik")
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "IDApTIK does not declare a package compiler".to_owned())?;
            let package = compiler
                .compile(&source, &idaptik_root)
                .map_err(|error| error.to_string())?;
            ums_profiles::write_pretty_json(&output, &package)
                .map_err(|error| error.to_string())?;
            Ok(format!("compiled {}", output.display()))
        }
        Command::Validate { profile, fixture } => {
            ums_profiles::read_and_validate_declared_profile(&profile, &fixture)
                .map_err(|error| error.to_string())?;
            Ok(format!("valid {}", fixture.display()))
        }
        Command::Preview {
            runner,
            validator,
            policies,
            mut runner_args,
            runner_output_flag,
            mind_trace_flag,
            validator_args,
            timeout_ms,
            explain,
        } => {
            if let Some(flag) = runner_output_flag {
                runner_args.extend([flag, RUNNER_OUTPUT_SLOT.to_owned()]);
            }
            if let Some(flag) = mind_trace_flag {
                runner_args.extend([flag, MIND_TRACE_SLOT.to_owned()]);
            }
            let adapter = ProcessPreviewAdapter::new(runner, validator)
                .with_policies(policies)
                .with_runner_args(runner_args)
                .with_validator_args(validator_args)
                .with_timeout(Duration::from_millis(timeout_ms));
            preview(&adapter, explain)
        }
    }
}

fn preview(adapter: &ProcessPreviewAdapter, explain: bool) -> Result<String, String> {
    let outcome = adapter.run(None).map_err(|error| error.to_string())?;
    if explain {
        let mut documents = vec![
            outcome
                .runner_output_document()
                .map_err(|error| error.to_string())?,
        ];
        documents.extend(
            outcome
                .mind_trace_document()
                .map_err(|error| error.to_string())?,
        );
        let report = ums_profile_sdk::explain::explain_documents(&documents, true)
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "replay-identical: ok\nvalidator: accepted (exit 0)\n{}",
            ums_profile_sdk::explain::render_text(&report).trim_end()
        ))
    } else {
        let response = outcome.response().map_err(|error| error.to_string())?;
        ums_profile_sdk::canonical::to_canonical_string(&response)
            .map_err(|error| error.to_string())
    }
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("profile error: {error}");
            ExitCode::FAILURE
        }
    }
}
