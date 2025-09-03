use anyhow::{Context as _, Ok, Result};
use chrono::{DateTime, Local, Timelike as _};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use uuid::Uuid;

use crate::daemon::UnblockingDaemon;

const DAEMON_ID_PATH: &str = "/tmp/enough/daemon_id";
const STATE_BACKUP_PATH: &str = "/tmp/enough/current_block.yaml";

pub struct SystemdDaemon;

impl UnblockingDaemon for SystemdDaemon {
    fn schedule(unblock_time: DateTime<Local>) -> Result<()> {
        let daemon_id = format!("enough-unblock-{}", Uuid::new_v4());
        let service_path = Self::get_service_path(&daemon_id)?;

        let current_exe = env::current_exe().context("Failed to get current executable path")?;
        let service_content = Self::generate_service(&current_exe, unblock_time);

        fs::write(&service_path, service_content).with_context(|| {
            format!("Failed to write service file to {}", service_path.display())
        })?;

        let output = Command::new("systemctl")
            .arg("--user")
            .arg("enable")
            .arg(&service_path)
            .output()
            .context("Failed to execute systemctl enable command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("systemctl enable failed: {}", stderr);
        }

        let output = Command::new("systemctl")
            .arg("--user")
            .arg("start")
            .arg(&daemon_id)
            .output()
            .context("Failed to execute systemctl start command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("systemctl start failed: {}", stderr);
        }

        // saving daemon info for cleanup
        fs::create_dir_all("/tmp/enough")?;
        fs::write(DAEMON_ID_PATH, &daemon_id)?;

        Ok(())
    }

    fn remove() -> Result<()> {
        if Path::new(DAEMON_ID_PATH).exists() {
            let daemon_id = fs::read_to_string(DAEMON_ID_PATH)?;
            let service_path = Self::get_service_path(&daemon_id.trim())?;

            fs::remove_file(DAEMON_ID_PATH)?;
            fs::remove_file(STATE_BACKUP_PATH)?;

            // unloading the daemon
            let output = Command::new("systemctl")
                .arg("--user")
                .arg("stop")
                .arg(&daemon_id)
                .output()
                .context("Failed to execute systemctl stop command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("Warning: systemctl stop failed: {}", stderr);
            }

            let output = Command::new("systemctl")
                .arg("--user")
                .arg("disable")
                .arg(&service_path)
                .output()
                .context("Failed to execute systemctl disable command")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!("Warning: systemctl disable failed: {}", stderr);
            }

            if service_path.exists() {
                fs::remove_file(&service_path).with_context(|| {
                    format!("Failed to remove service file {}", service_path.display())
                })?;
            }

            eprintln!("Removed scheduled unblock");
        }
        Ok(())
    }
}

impl SystemdDaemon {
    fn get_service_path(daemon_id: &str) -> Result<PathBuf> {
        let home_dir = env::var("HOME").context("Failed to get HOME environment variable")?;
        let service_dir = Path::new(&home_dir).join(".config/systemd/user");
        fs::create_dir_all(&service_dir)
            .with_context(|| format!("Failed to create directory {}", service_dir.display()))?;
        Ok(service_dir.join(format!("{}.service", daemon_id)))
    }

    fn generate_service(exec_path: &Path, unblock_time: DateTime<Local>) -> String {
        let (hour, minute) = (unblock_time.hour(), unblock_time.minute());
        format!(
            "[Unit]
Description=Enough Unblock Daemon
After=network.target
[Service]
Type=oneshot
ExecStart={} ___zzzunblock --fix
[Install]
WantedBy=default.target
[Timer]
OnCalendar=*-*-* {}:{}:00
Persistent=true
",
            exec_path.display(),
            hour,
            minute
        )
    }
}
