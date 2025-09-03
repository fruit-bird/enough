#[cfg(target_os = "macos")]
mod macos;

use anyhow::Result;
use chrono::{DateTime, Local};

#[cfg(target_os = "macos")]
pub use macos::LaunchDaemon as EnoughDaemon;

/// Trait defining the interface for scheduling and removing unblocking daemons.
/// This trait is implemented differently for macOS and Linux due to their distinct
/// approaches to background services.
pub trait UnblockingDaemon {
    /// Schedules a daemon to unblock at the specified time.
    fn schedule(unblock_time: DateTime<Local>) -> Result<()>;

    /// Removes the scheduled daemon.
    fn remove() -> Result<()>;
}
