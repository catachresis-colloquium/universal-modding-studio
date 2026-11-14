// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2025-2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use ums_profile_sdk::ProfileRegistry;
use ums_profile_sdk::preview::ProcessPreviewAdapter;

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
    Preview {
        /// Explicit path to the runner binary.
        #[arg(long)]
        runner: PathBuf,
        /// Explicit path to the validator binary.
        #[arg(long)]
        validator: PathBuf,
        /// One runner argument; repeat for more. Passed as an argument array.
        #[arg(long = "runner-arg", allow_hyphen_values = true)]
        runner_args: Vec<String>,
        /// One validator argument; repeat for more.
        #[arg(long = "validator-arg", allow_hyphen_values = true)]
        validator_args: Vec<String>,
        /// Per-process timeout in milliseconds.
        #[arg(long, default_value_t = 60_000)]
        timeout_ms: u64,
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
            runner_args,
            validator_args,
            timeout_ms,
        } => preview(runner, validator, runner_args, validator_args, timeout_ms),
    }
}

fn preview(
    runner: PathBuf,
    validator: PathBuf,
    runner_args: Vec<String>,
    validator_args: Vec<String>,
    timeout_ms: u64,
) -> Result<String, String> {
    let outcome = ProcessPreviewAdapter::new(runner, validator)
        .with_runner_args(runner_args)
        .with_validator_args(validator_args)
        .with_timeout(Duration::from_millis(timeout_ms))
        .run(None)
        .map_err(|error| error.to_string())?;
    {
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
