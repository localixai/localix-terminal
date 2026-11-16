//! How long a command the person typed must have run for the window to tell them it finished
//! while they were not looking (Settings, Terminal): 30 seconds by default, or never.

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::browser) enum CommandNotice {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "10s")]
    After10s,
    #[default]
    #[serde(rename = "30s")]
    After30s,
    #[serde(rename = "1m")]
    After1m,
    #[serde(rename = "5m")]
    After5m,
}

impl CommandNotice {
    /// The least a command runs to be told; none: never told.
    pub(in crate::browser) fn after(self) -> Option<Duration> {
        let seconds = match self {
            CommandNotice::Off => return None,
            CommandNotice::After10s => 10,
            CommandNotice::After30s => 30,
            CommandNotice::After1m => 60,
            CommandNotice::After5m => 300,
        };
        Some(Duration::from_secs(seconds))
    }
}
