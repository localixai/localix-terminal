//! The Claude Code plugin, from the app itself: the bundle holds it as a marketplace
//! (`Contents/Resources/claude-code`, scripts/bundle-macos.sh), which Claude Code is told of and
//! installs from, with its own `claude plugin` commands. The plugin is the app's version, so the
//! app refreshes it when it was updated.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::State;
use super::shell::{Ran, login_path, run, which};

/// The plugin as Claude Code names it, and its marketplace.
const PLUGIN: &str = "llx-desk@llx-desk";
const MARKETPLACE: &str = "llx-desk";
/// How long a `claude plugin` command may take (an install copies the plugin).
const MOST: Duration = Duration::from_secs(90);

/// The marketplace in the app's bundle, when this program is the app's.
fn marketplace() -> Option<PathBuf> {
    let dir = crate::bundle::app()?.join("Contents/Resources/claude-code");
    dir.join(".claude-plugin/marketplace.json")
        .is_file()
        .then_some(dir)
}

/// `claude`, and the PATH to run it with.
fn claude() -> Result<(PathBuf, String), String> {
    let path = login_path().ok_or("the login shell did not say its PATH")?;
    let claude =
        which("claude", &path).ok_or("Claude Code is not installed (no `claude` on the PATH)")?;
    Ok((claude, path))
}

fn ask(claude: &(PathBuf, String), args: &[&str]) -> Result<Ran, String> {
    run(&claude.0, args, &claude.1, MOST)
}

/// The plugin's version Claude Code has, if it has it enabled.
fn installed(claude: &(PathBuf, String)) -> Result<Option<String>, String> {
    let listed = ask(claude, &["plugin", "list", "--json"])?;
    if !listed.ok {
        return Err(listed.out);
    }
    let plugins: Value = serde_json::from_str(listed.out.trim()).map_err(|why| why.to_string())?;
    let mine = (plugins.as_array().into_iter().flatten())
        .find(|plugin| plugin["id"] == PLUGIN && plugin["enabled"] == true);
    Ok(mine.map(|plugin| plugin["version"].as_str().unwrap_or_default().to_string()))
}

pub(super) fn state() -> State {
    if marketplace().is_none() {
        return State::Unavailable("from LLX Desk.app: its bundle holds the plugin".into());
    }
    let claude = match claude() {
        Ok(claude) => claude,
        Err(why) => return State::Unavailable(why),
    };
    match installed(&claude) {
        Ok(Some(version)) if version == env!("CARGO_PKG_VERSION") => {
            State::On(format!("the plugin {version} is installed"))
        }
        Ok(Some(version)) => State::Off(format!("the plugin {version} is older than the app")),
        Ok(None) => State::Off("not installed".into()),
        Err(why) => State::Unavailable(why),
    }
}

/// The plugin installed, of the app's version: its marketplace pointed at the bundle (an earlier
/// one of the same name, elsewhere, is replaced), then installed or updated.
pub(super) fn turn_on() -> Result<String, String> {
    let dir = marketplace().ok_or("only LLX Desk.app holds the plugin")?;
    let claude = claude()?;
    if !points_at(&claude, &dir) {
        let _ = ask(&claude, &["plugin", "marketplace", "remove", MARKETPLACE]);
        let added = ask(
            &claude,
            &["plugin", "marketplace", "add", &dir.display().to_string()],
        )?;
        if !added.ok {
            return Err(added.out);
        }
    } else {
        let _ = ask(&claude, &["plugin", "marketplace", "update", MARKETPLACE]);
    }
    let fresh =
        |claude| installed(claude).ok().flatten().as_deref() == Some(env!("CARGO_PKG_VERSION"));
    if !fresh(&claude) {
        let _ = ask(&claude, &["plugin", "install", PLUGIN]);
    }
    if !fresh(&claude) {
        let updated = ask(&claude, &["plugin", "update", PLUGIN])?;
        if !fresh(&claude) {
            return Err(updated.out);
        }
    }
    Ok("the plugin is installed: start a new Claude Code session to use it".into())
}

/// The plugin and its marketplace gone from Claude Code.
pub(super) fn turn_off() -> Result<String, String> {
    let claude = claude()?;
    let removed = ask(&claude, &["plugin", "uninstall", PLUGIN])?;
    let _ = ask(&claude, &["plugin", "marketplace", "remove", MARKETPLACE]);
    match removed.ok {
        true => Ok("the plugin is uninstalled".into()),
        false => Err(removed.out),
    }
}

/// Whether Claude Code's marketplace of the name is `dir`.
fn points_at(claude: &(PathBuf, String), dir: &Path) -> bool {
    let Ok(listed) = ask(claude, &["plugin", "marketplace", "list", "--json"]) else {
        return false;
    };
    let markets: Value = serde_json::from_str(listed.out.trim()).unwrap_or_default();
    (markets.as_array().into_iter().flatten()).any(|market| {
        market["name"] == MARKETPLACE && market["path"].as_str() == Some(&dir.display().to_string())
    })
}
