//! The app's update as the window words it: the row of Settings, and the offer under the strip
//! once one is ready. Pure: `update.rs` owns how far an update is, and nothing here keeps it.

use serde::Serialize;

use crate::update::{Stage, Told};

/// The ids of the offers of an update: this, then its version.
pub(super) const OF_UPDATE: &str = "update:";

/// What the app menu's Check for Updates… says, as a click of the strip is said.
pub(super) const CHECK: &str = "update check";

/// Whether the app looks for its updates itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Updates {
    /// Checked for and downloaded while the person works; installed when they say so.
    #[default]
    Auto,
    /// Only when they ask.
    Manual,
}

/// What the row's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Act {
    Check,
    Apply,
}

/// The row of Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Row {
    /// The build that runs: `Localix Terminal 0.1.5`.
    pub(super) title: String,
    pub(super) says: String,
    /// How much of a download is there, in hundredths; none when nothing is fetched, or its
    /// size is not known.
    pub(super) progress: Option<u8>,
    /// Something is under way: the button waits.
    pub(super) busy: bool,
    pub(super) failed: bool,
    pub(super) act: Option<Act>,
    /// The button's words.
    pub(super) button: Option<&'static str>,
    /// The version whose notes can be read, once a newer one is known.
    pub(super) notes: Option<String>,
}

/// How long ago `then` was, as of `now` (seconds of the epoch), in words.
fn ago(then: u64, now: u64) -> String {
    let minutes = now.saturating_sub(then) / 60;
    match minutes {
        0 => "just now".into(),
        1 => "a minute ago".into(),
        2..=59 => format!("{minutes} minutes ago"),
        60..=119 => "an hour ago".into(),
        120..=1439 => format!("{} hours ago", minutes / 60),
        1440..=2879 => "yesterday".into(),
        _ => format!("{} days ago", minutes / 1440),
    }
}

/// The row for what the updater tells, as of `now`.
pub(super) fn row(told: &Told, now: u64) -> Row {
    let version = told.current.split('+').next().unwrap_or(&told.current);
    let mut row = Row {
        title: format!("Localix Terminal {version}"),
        says: String::new(),
        progress: None,
        busy: false,
        failed: false,
        act: Some(Act::Check),
        button: Some("Check Now"),
        notes: None,
    };
    if let Some(why) = &told.unable {
        row.says.clone_from(why);
        (row.act, row.button) = (None, None);
        return row;
    }
    match &told.stage {
        Stage::Idle => {
            row.says = match told.checked {
                Some(checked) => format!("Up to date. Checked {}.", ago(checked, now)),
                None => "Not checked yet.".into(),
            };
        }
        Stage::Checking => {
            row.says = "Checking for updates…".into();
            row.busy = true;
        }
        Stage::Fetching { release, got, of } => {
            let of = of.filter(|of| *of > 0);
            let part = of.map(|of| (got.saturating_mul(100) / of).min(100));
            row.progress = part.and_then(|part| u8::try_from(part).ok());
            row.says = match row.progress {
                Some(part) => format!("Downloading {}… {part}%", release.version),
                None => format!("Downloading {}…", release.version),
            };
            row.busy = true;
            row.notes = Some(release.version.to_string());
        }
        Stage::Ready { release } => {
            row.says = format!("{} is ready to install.", release.version);
            (row.act, row.button) = (Some(Act::Apply), Some("Restart to Update"));
            row.notes = Some(release.version.to_string());
        }
        Stage::Failed { why } => {
            row.says = format!("The update did not go through: {why}");
            row.failed = true;
            row.button = Some("Try Again");
        }
    }
    row
}

/// The offer for what the updater tells: its id (of the version, so one put off is offered
/// again with the next) and its words. Only an update that is ready is offered.
pub(super) fn offer(told: &Told) -> Option<(String, String)> {
    let Stage::Ready { release } = &told.stage else {
        return None;
    };
    Some((
        format!("{OF_UPDATE}{}", release.version),
        format!("Localix Terminal {} is ready to install.", release.version),
    ))
}

/// The announcement of an update that is ready: its offer's id, and the dialog that names both
/// versions and lists what the release says is new. Its own id is of the version, so the next
/// version is announced anew.
pub(super) fn announcement(told: &Told) -> Option<(String, super::dialog::Dialog)> {
    let (offered, _) = offer(told)?;
    let Stage::Ready { release } = &told.stage else {
        return None;
    };
    let dialog = super::dialog::Dialog {
        id: format!("announced:{}", release.version),
        mark: "app",
        tone: "accent",
        versions: Some((told.current.clone(), release.version.to_string())),
        title: format!("Localix Terminal {} is ready", release.version),
        detail: format!(
            "You have {}. Restarting installs the update; if a terminal still runs a program, \
             you are asked first.",
            told.current
        ),
        heading: "What’s New".into(),
        notes: super::dialog::notes(&release.notes),
        fields: vec![],
        yes: "Restart to Update".into(),
        no: "Later".into(),
        asked: false,
    };
    Some((offered, dialog))
}

/// The page of a release's notes.
pub(super) fn notes(version: &str) -> String {
    let repo = crate::update::REPO;
    format!("https://github.com/{repo}/releases/tag/v{version}")
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
