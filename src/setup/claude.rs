//! The Claude Code plugin, from the app itself: the bundle holds it as a marketplace
//! (`Contents/Resources/claude-code`, scripts/bundle-macos.sh), which Claude Code is told of and
//! installs from, with its own `claude plugin` commands. The plugin is the app's version, so the
//! app refreshes it when it was updated.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::State;
use super::former::Swept;
use super::shell::{Ran, failure, find, login_path, run};

/// The plugin as Claude Code names it, and its marketplace.
const PLUGIN: &str = "llxterm@llxterm";
const MARKETPLACE: &str = "llxterm";
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
    Ok(plugin_in(&plugins))
}

/// The plugin in `plugins` (Claude Code's list): the version a session loads, and whether it is
/// enabled. Read from a folder, as from the app's bundle, that is the folder's version, which
/// Claude Code names beside the one it recorded at the install.
fn plugin_in(plugins: &Value) -> Option<(String, bool)> {
    let mine = (plugins.as_array()?.iter()).find(|plugin| plugin["id"] == PLUGIN)?;
    let version = (mine["folderVersion"].as_str())
        .or_else(|| mine["version"].as_str())
        .unwrap_or_default();
    Some((version.to_string(), mine["enabled"] == true))
}

/// Claude Code's own folder, as this process's variables name it.
fn records() -> Option<PathBuf> {
    Some(dir(&super::home().ok()?, |name| std::env::var(name).ok()))
}

/// Claude Code's own folder for the person whose home is `home` and whose variables `var`
/// reads: `CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub(super) fn dir(home: &Path, var: impl Fn(&str) -> Option<String>) -> PathBuf {
    var("CLAUDE_CONFIG_DIR")
        .filter(|dir| !dir.trim().is_empty())
        .map_or_else(|| home.join(".claude"), PathBuf::from)
}

/// Whether Claude Code's records, read without starting it, say the plugin is in place.
/// `claude plugin list` reads every plugin the person has, wherever it is kept, and macOS asks
/// the person about each protected folder in the app's name; so it is run only when the
/// records leave something to repair or to ask.
fn recorded(dir: &Path) -> bool {
    let read = |name: &str| -> Option<Value> {
        serde_json::from_str(&std::fs::read_to_string(records()?.join(name)).ok()?).ok()
    };
    let (Some(installed), Some(markets), Some(settings)) = (
        read("plugins/installed_plugins.json"),
        read("plugins/known_marketplaces.json"),
        read("settings.json"),
    ) else {
        return false;
    };
    on_record(&installed, &markets, &settings, dir)
}

/// The plugin in Claude Code's records: installed at the app's version, from the marketplace
/// at `dir`, and enabled.
fn on_record(installed: &Value, markets: &Value, settings: &Value, dir: &Path) -> bool {
    let version = env!("CARGO_PKG_VERSION");
    let current = (installed["plugins"][PLUGIN].as_array())
        .is_some_and(|copies| copies.iter().any(|copy| copy["version"] == version));
    let here = markets[MARKETPLACE]["source"]["path"].as_str() == Some(&dir.display().to_string());
    current && here && settings["enabledPlugins"][PLUGIN] == true
}

pub(super) fn state() -> State {
    let Some(dir) = marketplace() else {
        return State::Unavailable("from Localix Terminal.app: its bundle holds the plugin".into());
    };
    if recorded(&dir) {
        let version = env!("CARGO_PKG_VERSION");
        return State::On(format!("the plugin {version} is installed"));
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
    let dir = marketplace().ok_or("only Localix Terminal.app holds the plugin")?;
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

/// LLX Desk 0.1.6's plugin and its marketplace, as Claude Code names them.
const FORMER_PLUGIN: &str = "llx-desk@llx-desk";
const FORMER_MARKETPLACE: &str = "llx-desk";

/// What Claude Code's records hold of LLX Desk's plugin and marketplace.
#[derive(Debug, PartialEq, Eq)]
enum Former {
    /// Neither.
    Absent,
    /// One of them: the folder of the marketplace, when they name one.
    Recorded(Option<PathBuf>),
}

/// What Claude Code's records (its installed plugins, its known marketplaces and its settings)
/// hold of LLX Desk's plugin and marketplace.
fn former_in(installed: &Value, markets: &Value, settings: &Value) -> Former {
    let folder = |market: &Value| market["source"]["path"].as_str().map(PathBuf::from);
    let market = (markets.get(FORMER_MARKETPLACE))
        .or_else(|| settings["extraKnownMarketplaces"].get(FORMER_MARKETPLACE));
    let plugin = installed["plugins"].get(FORMER_PLUGIN).is_some()
        || settings["enabledPlugins"].get(FORMER_PLUGIN).is_some();
    match market {
        Some(market) => Former::Recorded(folder(market)),
        None if plugin => Former::Recorded(None),
        None => Former::Absent,
    }
}

/// Whether `dir` holds LLX Desk's marketplace still (an LLX Desk installed beside this app).
fn holds_former(dir: &Path) -> bool {
    let manifest = std::fs::read_to_string(dir.join(".claude-plugin/marketplace.json"));
    (manifest
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok()))
    .is_some_and(|market| market["name"] == FORMER_MARKETPLACE)
}

/// LLX Desk's plugin and marketplace taken out of Claude Code with its own commands, when its
/// records hold them and the marketplace's folder no longer holds LLX Desk's (0.1.6's updater
/// replaced the app, or this app took its place).
pub(super) fn retire() -> Swept {
    let Some(dir) = records() else {
        return Swept::Nothing;
    };
    let read = |name: &str| -> Value {
        (std::fs::read_to_string(dir.join(name)).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    };
    let found = former_in(
        &read("plugins/installed_plugins.json"),
        &read("plugins/known_marketplaces.json"),
        &read("settings.json"),
    );
    let Former::Recorded(market) = found else {
        return Swept::Nothing;
    };
    if let Some(market) = market.filter(|market| holds_former(market)) {
        return Swept::Left(format!("its marketplace at {} is there", market.display()));
    }
    let claude = match find("claude") {
        Ok(claude) => claude,
        Err(why) if login_path().is_some() => return Swept::Left(why),
        Err(why) => return Swept::Failed(why),
    };
    // Either may have nothing to take out (a marketplace with no plugin installed from it).
    let plugin = ask(&claude, &["plugin", "uninstall", FORMER_PLUGIN]);
    let market = ask(
        &claude,
        &["plugin", "marketplace", "remove", FORMER_MARKETPLACE],
    );
    match (plugin, market) {
        (Ok(plugin), Ok(market)) if !plugin.ok && !market.ok => Swept::Failed(failure(
            &market,
            "`claude plugin marketplace remove llx-desk` failed and said nothing",
        )),
        (Ok(_), Ok(_)) => {
            Swept::Done("the plugin llx-desk and its marketplace are taken out".into())
        }
        (Err(why), _) | (_, Err(why)) => Swept::Failed(why),
    }
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
