#![cfg(target_os = "macos")]

use anyhow::{Context as _, Ok, Result};
use chrono::{DateTime, Local, Timelike as _};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use uuid::Uuid;

const DAEMON_INFO_PATH: &str = "/tmp/enough/daemon_id";
const STATE_BACKUP_PATH: &str = "/tmp/enough/current_block.yaml";

pub struct LaunchDaemon;

impl LaunchDaemon {
    pub fn create_unblock_daemon(unblock_time: DateTime<Local>) -> Result<()> {
        let daemon_id = format!("com.enough.unblock.{}", Uuid::new_v4());
        let plist_path = Self::get_plist_path(&daemon_id)?;

        let current_exe = env::current_exe().context("Failed to get current executable path")?;
        let plist_content = Self::generate_plist(&daemon_id, &current_exe, unblock_time);

        fs::write(&plist_path, plist_content)
            .with_context(|| format!("Failed to write plist file to {}", plist_path.display()))?;

        let uid = env::var("UID").unwrap_or_else(|_| "501".to_string());
        let output = Command::new("launchctl")
            .arg("bootstrap")
            .arg(format!("gui/{}", uid))
            .arg(&plist_path)
            .output()
            .context("Failed to execute launchctl load command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("launchctl load failed: {}", stderr);
        }

        eprint!("Scheduled unblock for ");
        println!("{}", unblock_time.format("%H:%M:%S"));

        // saving daemon info for cleanup
        fs::create_dir_all("/tmp/enough")?;
        fs::write(DAEMON_INFO_PATH, &daemon_id)?;

        Ok(())
    }

    pub fn remove() -> Result<()> {
        if Path::new(DAEMON_INFO_PATH).exists() {
            let daemon_id = fs::read_to_string(DAEMON_INFO_PATH)?;
            let plist_path = Self::get_plist_path(&daemon_id.trim())?;

            fs::remove_file(DAEMON_INFO_PATH)?;
            fs::remove_file(STATE_BACKUP_PATH)?;

            // unloading the daemon
            let uid = env::var("UID").unwrap_or_else(|_| "501".to_string());
            let output = Command::new("launchctl")
                .arg("bootout")
                .arg(format!("gui/{}", uid))
                .arg(&plist_path)
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("launchctl unload failed: {}", stderr);
            }

            fs::remove_file(&plist_path)?;
        }

        Ok(())
    }

    fn get_plist_path(daemon_id: &str) -> Result<PathBuf> {
        // Going with this approach rather than fetching $HOME because when run with sudo,
        // $HOME points to /var/root which messes up the path to LaunchAgents
        let home_dir_bytes = Command::new("realpath")
            .arg("~")
            .output()
            .context("Failed to get home directory using realpath")?
            .stdout;
        let home_dir = String::from_utf8_lossy(&home_dir_bytes);
        let launch_agents_dir = format!("{}/Library/LaunchAgents", home_dir.trim());
        let plist_path = Path::new(&launch_agents_dir).join(format!("{}.plist", daemon_id));

        Ok(plist_path)
    }

    fn generate_plist(
        daemon_id: &str,
        executable_path: &Path,
        unblock_time: DateTime<Local>,
    ) -> String {
        let start_calendar_interval = format!(
            "    <dict>
        <key>Hour</key>
        <integer>{}</integer>
        <key>Minute</key>
        <integer>{}</integer>
        <key>Second</key>
        <integer>{}</integer>
    </dict>",
            unblock_time.hour(),
            unblock_time.minute(),
            unblock_time.second(),
        );

        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>sudo</string>
        <string>{}</string>
        <string>___zzzunblock</string>
        <string>--fix</string>
    </array>
    <key>StartCalendarInterval</key>
{}
    <key>RunAtLoad</key>
    <false/>
    <key>StandardOutPath</key>
    <string>/tmp/enough/unblock.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/enough/unblock.log</string>
</dict>
</plist>"#,
            daemon_id,
            executable_path.display(),
            start_calendar_interval
        );

        plist
    }
}
