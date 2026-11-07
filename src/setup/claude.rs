//! The Claude Code plugin, from the app itself: the bundle holds it as a marketplace
//! (`Contents/Resources/claude-code`, scripts/bundle-macos.sh), which Claude Code is told of and
//! installs from, with its own `claude plugin` commands. The plugin is the app's version, so the
//! app refreshes it when it was updated.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::State;
use super::shell::{Ran, failure, find, run};

/// The plugin as Claude Code names it, and its marketplace.
const PLUGIN: &str = "llx-desk@llx-desk";
const MARKETPLACE: &str = "llx-desk";
/// How long a `claude plugin` command may take (an install copies the plugin).
const MOST: Duration = Duration::from_secs(90);
/// Why the plugin is off when the person disabled it in Claude Code: the app leaves it so.
pub(super) const DISABLED: &str = "disabled in Claude Code";

/// The marketplace in the app's bundle, when this program is the app's.
fn marketplace() -> Option<PathBuf> {
    let dir = crate::bundle::app()?.join("Contents/Resources/claude-code");
    dir.join(".claude-plugin/marketplace.json")
        .is_file()
        .then_some(dir)
}

fn ask(claude: &(PathBuf, String), args: &[&str]) -> Result<Ran, String> {
    run(&claude.0, args, &claude.1, MOST)
}

/// The plugin's version Claude Code has, and whether it has it enabled.
fn installed(claude: &(PathBuf, String)) -> Result<Option<(String, bool)>, String> {
    let listed = ask(claude, &["plugin", "list", "--json"])?;
    if !listed.ok {
        return Err(failure(
            &listed,
            "`claude plugin list` failed and said nothing",
        ));
    }
    let plugins: Value = serde_json::from_str(listed.out.trim()).map_err(|why| why.to_string())?;
    let mine = (plugins.as_array().into_iter().flatten()).find(|plugin| plugin["id"] == PLUGIN);
    Ok(mine.map(|plugin| {
        let version = plugin["version"].as_str().unwrap_or_default().to_string();
        (version, plugin["enabled"] == true)
    }))
}

pub(super) fn state() -> State {
    if marketplace().is_none() {
        return State::Unavailable("from LLX Desk.app: its bundle holds the plugin".into());
    }
    let claude = match find("claude") {
        Ok(claude) => claude,
        Err(why) => return State::Unavailable(why),
    };
    match installed(&claude) {
        Ok(Some((_, false))) => State::Off(DISABLED.into()),
        Ok(Some((version, true))) if version == env!("CARGO_PKG_VERSION") => {
            State::On(format!("the plugin {version} is installed"))
        }
        Ok(Some((version, true))) => {
            State::Off(format!("the plugin {version} is older than the app"))
        }
        Ok(None) => State::Off("not installed".into()),
        Err(why) => State::Unavailable(why),
    }
}

/// The plugin installed and enabled, of the app's version: its marketplace pointed at the bundle
/// (an earlier one of the same name, elsewhere, is replaced), then installed or updated.
pub(super) fn turn_on() -> Result<String, String> {
    let dir = marketplace().ok_or("only LLX Desk.app holds the plugin")?;
    let claude = find("claude")?;
    // Taken out only when it is known to name another folder: a list that could not be read
    // (it timed out, Claude Code said another thing) must not remove the plugin with it.
    let at = market(&claude, &dir);
    match at {
        Market::Elsewhere | Market::Absent => {
            if at == Market::Elsewhere {
                let _ = ask(&claude, &["plugin", "marketplace", "remove", MARKETPLACE]);
            }
            let added = ask(
                &claude,
                &["plugin", "marketplace", "add", &dir.display().to_string()],
            )?;
            if !added.ok {
                return Err(failure(
                    &added,
                    "`claude plugin marketplace add` failed and said nothing",
                ));
            }
        }
        Market::Here | Market::Unknown => {
            let _ = ask(&claude, &["plugin", "marketplace", "update", MARKETPLACE]);
        }
    }
    let fresh = |claude| {
        installed(claude).ok().flatten() == Some((env!("CARGO_PKG_VERSION").to_string(), true))
    };
    // Disabled in Claude Code, and the person asks for it: enabled again.
    if matches!(installed(&claude), Ok(Some((_, false)))) {
        let _ = ask(&claude, &["plugin", "enable", PLUGIN]);
    }
    if !fresh(&claude) {
        let _ = ask(&claude, &["plugin", "install", PLUGIN]);
    }
    if !fresh(&claude) {
        let updated = ask(&claude, &["plugin", "update", PLUGIN])?;
        if !fresh(&claude) {
            return Err(failure(
                &updated,
                "Claude Code still lists another version of the plugin, or has it disabled: run /reload-plugins in it, or start it again",
            ));
        }
    }
    Ok("the plugin is installed: start a new Claude Code session to use it".into())
}

/// The plugin and its marketplace gone from Claude Code.
pub(super) fn turn_off() -> Result<String, String> {
    let claude = find("claude")?;
    let removed = ask(&claude, &["plugin", "uninstall", PLUGIN])?;
    let _ = ask(&claude, &["plugin", "marketplace", "remove", MARKETPLACE]);
    match removed.ok {
        true => Ok("the plugin is uninstalled".into()),
        false => Err(failure(
            &removed,
            "`claude plugin uninstall` failed and said nothing",
        )),
    }
}

/// Where Claude Code's marketplace of the name is.
#[derive(Debug, PartialEq, Eq)]
enum Market {
    /// At the app's folder.
    Here,
    /// At another folder.
    Elsewhere,
    /// Not listed.
    Absent,
    /// The list could not be read.
    Unknown,
}

/// Where Claude Code's marketplace of the name is, as it lists them.
fn market(claude: &(PathBuf, String), dir: &Path) -> Market {
    let Ok(listed) = ask(claude, &["plugin", "marketplace", "list", "--json"]) else {
        return Market::Unknown;
    };
    match serde_json::from_str::<Value>(listed.out.trim()) {
        Ok(markets) if listed.ok => marketplace_in(&markets, dir),
        _ => Market::Unknown,
    }
}

/// Where the marketplace of the name is in `markets` (Claude Code's list).
fn marketplace_in(markets: &Value, dir: &Path) -> Market {
    let Some(list) = markets.as_array() else {
        return Market::Unknown;
    };
    match list.iter().find(|market| market["name"] == MARKETPLACE) {
        None => Market::Absent,
        Some(found) if found["path"].as_str() == Some(&dir.display().to_string()) => Market::Here,
        Some(_) => Market::Elsewhere,
    }
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
