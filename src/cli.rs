use anyhow::{Context, Ok, Result};
use chrono::Utc;
use clap::{Command, CommandFactory as _, Parser, Subcommand};
use humantime_serde::re::humantime::{format_duration, parse_duration};
use std::{
    env,
    fmt::Debug,
    io::{self, Write as _},
    path::PathBuf,
    time::Duration,
};

use crate::block::{BlockManager, Status};
use crate::config::EnoughConfig;

/// Enough overstimulation, take back control over your focus
#[derive(Debug, Parser)]
#[clap(
    version,
    about,
    author,
    long_about=None,
    after_help="You REALLY can't access these websites and apps for the specified duration, so make sure you won't need them",
)]
pub struct EnoughCLI {
    #[clap(subcommand)]
    command: EnoughOptions,
}

impl EnoughCLI {
    pub fn run(self) -> Result<()> {
        self.command.parse()
    }
}

#[derive(Debug, Subcommand)]
#[clap(rename_all = "kebab-case")]
enum EnoughOptions {
    /// Initialize by creating a sample config file
    Init {
        /// Path to the config file
        // #[clap(short, long, default_value = "~/.config/enough/enough.yaml")]
        #[clap(short, long)]
        output: Option<PathBuf>,
    },
    /// Block specified websites and apps
    Block {
        /// Path to the config file to use
        // #[clap(short, long, default_value = "enough.yaml")]
        #[clap(short, long)]
        config: Option<PathBuf>,
        /// Name of the profile to run
        #[clap(short, long)]
        profile: Option<String>,
        /// Override the duration set in the profile
        #[clap(short, long, value_parser = parse_duration)]
        duration: Option<Duration>,
    },
    /// (INTERNAL, DO NOT RUN MANUALLY) CLEANUP COMMAND.
    /// Rollback changes of the latest run in case of errors
    #[clap(hide = true, name = "___zzzunblock")]
    Unblock {
        #[clap(long, default_value = "false", hide = true)]
        fix: bool,
    },
    /// Show current status
    Status {
        /// Output in JSON format
        #[clap(long, default_value = "false")]
        json: bool,
        /// Output in a single line (for status bars)
        #[clap(long, default_value = "false", conflicts_with = "json")]
        line: bool,
    },
    /// List available profiles
    Profiles {
        #[clap(short, long)]
        config: Option<PathBuf>,
    },
    /// Generate shell completions
    Completions {
        /// The shell to generate the completions for
        shell: clap_complete::Shell,
    },
}

impl EnoughOptions {
    fn parse(self) -> Result<()> {
        match self {
            Self::Init { output } => {
                EnoughConfig::generate_sample(output.clone())
                    .with_context(|| "Failed to create config sample file")?;
            }
            Self::Block {
                config,
                profile,
                duration,
            } => {
                is_sudo()?;

                let block_manager = BlockManager::new();
                if block_manager.get_status(false)?.is_blocked() {
                    anyhow::bail!("A block is already active, please wait until it expires");
                }

                let conf = EnoughConfig::load(config)?;
                let profile_name = profile
                    .or_else(|| conf.default_profile.clone())
                    .with_context(
                        || "No profile specified and no default profile set in the config file",
                    )?;
                let profile = conf
                    .profiles
                    .get(&profile_name)
                    .with_context(|| format!("Profile `{}` not found", profile_name))?;
                let duration = duration.unwrap_or(profile.duration);

                let block_manager = BlockManager::new();
                block_manager.block_items(&profile_name, profile, duration)?;
            }
            Self::Unblock { fix } => {
                is_sudo()?;

                if fix {
                    let block_manager = BlockManager::new();
                    block_manager.unblock_all()?;
                    eprintln!("All items unblocked");
                } else {
                    eprintln!("This command is for internal use only, do NOT run it manually");
                }
            }
            Self::Status { json, line } => {
                let block_manager = BlockManager::new();
                if json {
                    let status = block_manager.get_status(false)?;
                    if status.is_blocked() {
                        let json = serde_json::to_string(&status)?;
                        println!("{:#}", json);
                    }
                } else if line {
                    let status = block_manager.get_status(false)?;
                    match status {
                        Status::Blocked {
                            profile_name,
                            unblock_time,
                        } => {
                            let now = Utc::now();
                            let remaining = unblock_time
                                .signed_duration_since(now)
                                .to_std()
                                .unwrap_or_default();
                            let remaining_secs = Duration::from_secs(remaining.as_secs());
                            print!("🔴 {} ({})", profile_name, format_duration(remaining_secs));
                        }
                        Status::Unblocked => print!("🟢 Unblocked"),
                    }
                    io::stdout().flush()?;
                } else {
                    block_manager.get_status(true)?;
                }
            }
            Self::Profiles { config } => {
                let conf = EnoughConfig::load(config)?;
                println!("{}", conf);
            }
            Self::Completions { shell } => {
                let cmd = EnoughCLI::command();
                let name = cmd.get_name().to_string();

                let mut filtered_cmd = Command::new(env!("CARGO_PKG_NAME"));
                for sub in cmd.get_subcommands() {
                    if sub.get_name() != "___zzzunblock" {
                        filtered_cmd = filtered_cmd.subcommand(sub);
                    }
                }

                clap_complete::generate(shell, &mut filtered_cmd, name, &mut io::stdout());
            }
        }

        Ok(())
    }
}

fn is_sudo() -> Result<()> {
    env::var("SUDO_USER").with_context(|| "This command must be run with sudo")?;
    Ok(())
}
