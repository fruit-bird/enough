use anyhow::{Context as _, Ok, Result};
use chrono::{DateTime, Local, Timelike as _};
use std::io::Write;
use std::{
    env, fs,
    os::unix::ffi::OsStrExt as _,
    path::{Path, PathBuf},
    process::Command,
};
use uuid::Uuid;

use crate::daemon::UnblockingDaemon;

const DAEMON_ID_PATH: &str = "/tmp/enough/daemon_id";
const STATE_BACKUP_PATH: &str = "/tmp/enough/current_block.yaml";
const HOME_DIR_BACKUP_PATH: &str = "/tmp/enough/home_dir";

pub struct LaunchDaemon;

impl UnblockingDaemon for LaunchDaemon {
    fn schedule(unblock_time: DateTime<Local>) -> Result<()> {
        let daemon_id = format!("com.enough.unblock.{}", Uuid::new_v4());
        let plist_path = Self::get_plist_path(&daemon_id, None)?;

        let current_exe = env::current_exe().context("Failed to get current executable path")?;
        let plist_content = Self::generate_plist(&daemon_id, &current_exe, unblock_time);

        fs::write(&plist_path, plist_content)
            .with_context(|| format!("Failed to write plist file to {}", plist_path.display()))?;

        let output = Command::new("launchctl")
            .arg("load")
            .arg(&plist_path)
            .output()
            .context("Failed to execute launchctl load command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("launchctl load failed: {}", stderr);
        }

        eprint!("Scheduled unblock for ");
        println!("{}", unblock_time.format("%H:%M:%S"));

        let home_dir = env::home_dir().with_context(|| "Couldn't find the home directory")?;
        eprintln!("backed up home dir: {}", home_dir.display());
        // saving daemon info for cleanup
        fs::create_dir_all("/tmp/enough")?;
        fs::write(DAEMON_ID_PATH, &daemon_id)?;
        fs::write(HOME_DIR_BACKUP_PATH, home_dir.as_os_str().as_bytes())?;

        Ok(())
    }

    fn remove() -> Result<()> {
        if Path::new(DAEMON_ID_PATH).exists() {
            let daemon_id = fs::read_to_string(DAEMON_ID_PATH)?;
            let home_dir = fs::read_to_string(HOME_DIR_BACKUP_PATH)?;
            eprintln!("restored home dir: {}", home_dir);
            let plist_path = Self::get_plist_path(&daemon_id.trim(), Some(home_dir.into()))?;
            eprintln!("restored plist path: {}", plist_path.display());

            fs::remove_file(DAEMON_ID_PATH)?;
            fs::remove_file(STATE_BACKUP_PATH)?;
            fs::remove_file(HOME_DIR_BACKUP_PATH)?;

            // unloading the daemon
            eprintln!("Unloading daemon with ID: {}", daemon_id);
            let output = Command::new("launchctl")
                .arg("unload")
                .arg(&plist_path)
                .output()?;
            eprintln!("Unloaded daemon with ID: {}", daemon_id);

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("launchctl unload failed: {}", stderr);
            }

            writeln!(
                std::fs::File::create("~/Downloads/daemon_unloaded")?,
                "Unloaded daemon with ID: {}",
                daemon_id
            )?;

            fs::remove_file(&plist_path)?;
        }

        Ok(())
    }
}

impl LaunchDaemon {
    fn get_plist_path(daemon_id: &str, home_dir: Option<PathBuf>) -> Result<PathBuf> {
        let home_dir = match home_dir {
            Some(home) => home,
            None => {
                let home_dir_bytes = env::home_dir().context("Couldn't find the home directory")?;
                let home_dir = String::from_utf8_lossy(&home_dir_bytes.as_os_str().as_bytes());
                home_dir.trim().to_string().into()
            }
        };

        let launch_agents_dir = format!("{}/Library/LaunchAgents", home_dir.display());
        let plist_path = Path::new(&launch_agents_dir).join(format!("{}.plist", daemon_id));

        eprintln!(
            "The plish path that will be used is: {}",
            plist_path.display()
        );

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
    <string>/tmp/enough/unblock.out</string>
    <key>StandardErrorPath</key>
    <string>/tmp/enough/unblock.err</string>
</dict>
</plist>"#,
            daemon_id,
            executable_path.display(),
            start_calendar_interval
        );

        plist
    }
}
