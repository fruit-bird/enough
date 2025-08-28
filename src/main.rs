mod block;
mod cli;
mod config;
mod daemon;

use clap::Parser as _;
use std::process::ExitCode;

use crate::cli::EnoughCLI;

#[cfg(not(target_os = "macos"))]
compile_error!("This application is currently only supported on macOS.");

fn main() -> ExitCode {
    let cli = EnoughCLI::parse();

    if let Err(e) = cli.run() {
        eprintln!("{:#}", e);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
