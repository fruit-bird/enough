use anyhow::{Context, Ok, Result};
use clap::{Command, CommandFactory as _, Parser, Subcommand};
use humantime_serde::re::humantime::parse_duration;
use std::io;
use std::{env, fmt::Debug, path::PathBuf, time::Duration};

use crate::block::BlockManager;
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
    Status,
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
                block_manager.block_items(profile, duration)?;
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
            Self::Status => {
                let block_manager = BlockManager::new();
                block_manager.get_status(true)?;
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
