use anyhow::{Context, Ok, Result};
use chrono::{DateTime, Local};
use humantime_serde::re::humantime::format_duration;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::Url;

use crate::{config::Profile, daemon::LaunchDaemon};

const HOSTS_FILE: &str = "/etc/hosts";
const ENOUGH_MARKER_START: &str = "# ENOUGH BLOCK START";
const ENOUGH_MARKER_END: &str = "# ENOUGH BLOCK END";
const ENOUGH_STATE_DIR: &str = "/tmp/enough";
const BLOCKED_APP_PERMS: &str = "000";
const UNBLOCKED_APP_PERMS: &str = "755";

pub struct BlockManager {
    pub state_dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct BlockState {
    profile_name: String,
    profile: Profile,
    unblock_time_secs: u64,
}

impl BlockManager {
    pub fn new() -> Self {
        Self {
            state_dir: PathBuf::from(ENOUGH_STATE_DIR),
        }
    }

    pub fn block_items(
        &self,
        profile_name: &str,
        profile: &Profile,
        duration: Duration,
    ) -> Result<()> {
        fs::create_dir_all(&self.state_dir)?; // Creating state directory
        self.unblock_all()?; // cleaning up any previous state

        if !profile.websites.is_empty() {
            Self::block_websites(&profile.websites)?;
        }

        // if !profile.apps.is_empty() {
        //     Self::block_apps(&profile.apps)?;
        // }

        let unblock_time = SystemTime::now() + duration;
        self.schedule_unblock(unblock_time.into())?;
        self.save_block_state(profile_name, profile, unblock_time)?;

        Ok(())
    }

    fn block_websites(websites: &[Url]) -> Result<()> {
        let hosts_file_contents = fs::read_to_string(HOSTS_FILE)?;
        let cleaned_content = Self::remove_existing_blocks(&hosts_file_contents);

        let mut new_content = cleaned_content;
        new_content.push_str(&format!("\n\n{}\n", ENOUGH_MARKER_START));
        for url in websites {
            if let Some(host) = url.host_str() {
                new_content.push_str(&format!("0.0.0.0 {}\n", host));
                new_content.push_str(&format!("::1 {}\n", host));

                if !host.contains("www.") {
                    new_content.push_str(&format!("0.0.0.0 www.{}\n", host));
                    new_content.push_str(&format!("::1 www.{}\n", host));
                } else {
                    // If host contains www., also block the non-www variant
                    let non_www = host.trim_start_matches("www.");
                    new_content.push_str(&format!("0.0.0.0 {}\n", non_www));
                    new_content.push_str(&format!("::1 {}\n", non_www));
                }
            }
        }
        new_content.push_str(&format!("{}\n\n", ENOUGH_MARKER_END));

        fs::write(HOSTS_FILE, new_content)?;

        let output = Command::new("sudo")
            .args(&["dscacheutil", "-flushcache"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to flush DNS cache: {}", stderr);
        }

        eprintln!("Blocked {} websites using hosts file", websites.len());

        Ok(())
    }

    fn block_apps(apps: &[PathBuf]) -> Result<()> {
        for app in apps {
            change_app_perms(app, BLOCKED_APP_PERMS)?;
        }

        todo!("App blocking not implemented yet");
    }

    pub fn unblock_all(&self) -> Result<()> {
        Self::unblock_websites()?;
        // Self::unblock_apps()?;

        // Removing launchd daemon
        LaunchDaemon::remove()?;

        // Cleaning up state
        fs::remove_dir_all(&self.state_dir)?;

        Ok(())
    }

    fn unblock_websites() -> Result<()> {
        let hosts_file_contents = fs::read_to_string(HOSTS_FILE)?;
        let cleaned_content = Self::remove_existing_blocks(&hosts_file_contents);
        fs::write(HOSTS_FILE, cleaned_content)?;

        let output = Command::new("sudo")
            .args(&["dscacheutil", "-flushcache"])
            .output()
            .with_context(|| "Failed to get output for DNS flushing command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to flush DNS cache: {}", stderr);
        }

        Ok(())
    }

    fn unblock_apps() -> Result<()> {
        todo!(
            "Need to backup each app's permissions before blocking in a file and restore from there"
        );
        // for app in apps {
        //     change_app_perms(app, UNBLOCKED_APP_PERMS)?;
        // }
    }

    fn remove_existing_blocks(content: &str) -> String {
        let lines = content.lines().collect::<Vec<_>>();
        let mut result = Vec::new();
        let mut in_block = false;

        for line in lines {
            if line.contains(ENOUGH_MARKER_START) {
                in_block = true;
                continue;
            }

            if line.contains(ENOUGH_MARKER_END) {
                in_block = false;
                continue;
            }

            if !in_block {
                result.push(line);
            }
        }

        result.join("\n")
    }

    fn schedule_unblock(&self, unblock_time: DateTime<Local>) -> Result<()> {
        LaunchDaemon::create_unblock_daemon(unblock_time)
    }

    fn save_block_state(
        &self,
        profile_name: &str,
        profile: &Profile,
        unblock_time: SystemTime,
    ) -> Result<()> {
        let state = BlockState {
            profile_name: profile_name.to_string(),
            profile: profile.clone(),
            unblock_time_secs: unblock_time.duration_since(UNIX_EPOCH)?.as_secs(),
        };

        let state_yml = serde_yml::to_string(&state)?;
        let state_file = self.state_dir.join("current_block.yaml");
        fs::write(state_file, state_yml)?;

        Ok(())
    }

    pub fn get_status(&self, print: bool) -> Result<Status> {
        let state_file = self.state_dir.join("current_block.yaml");

        if !state_file.exists() {
            if print {
                eprintln!("No active block is running");
            }
            return Ok(Status::Unblocked);
        }

        let state_content = fs::read_to_string(&state_file)?;
        let state = serde_yml::from_str::<BlockState>(&state_content)?;

        let unblock_time = UNIX_EPOCH + Duration::from_secs(state.unblock_time_secs);
        let now = SystemTime::now();

        if now < unblock_time && print {
            let remaining = unblock_time.duration_since(now)?;

            println!("Active block (profile: {})", state.profile_name);
            println!("• {} apps blocked", state.profile.apps.len());
            println!("• {} websites blocked", state.profile.websites.len());
            println!("• Time remaining: {}", format_duration(remaining));
        }

        Ok(Status::Blocked {
            profile_name: state.profile_name,
            unblock_time: unblock_time.into(),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Status {
    Blocked {
        profile_name: String,
        unblock_time: DateTime<Local>,
    },
    Unblocked,
}

impl Status {
    /// Returns `true` if the status is [`Blocked`].
    ///
    /// [`Blocked`]: Status::Blocked
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked { .. })
    }
}

fn change_app_perms(app: &PathBuf, perms: &str) -> Result<()> {
    let output = Command::new("sudo")
        .args(&["chmod", perms, app.to_str().unwrap()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to block app {:?}: {}", app, stderr);
    }

    Ok(())
}
