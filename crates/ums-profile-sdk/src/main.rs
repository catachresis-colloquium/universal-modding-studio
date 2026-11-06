// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2025-2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use ums_profile_sdk::ProfileRegistry;

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
