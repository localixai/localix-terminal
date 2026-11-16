//! Whether Claude Code takes a plugin folder from its variable, by its managed settings. An
//! organization can turn plugin folders off (`disableSideloadFlags`), and Claude Code then
//! refuses to start while `CLAUDE_CODE_PLUGIN_DIRS` names one. The sources are read as Claude
//! Code ranks them, highest first: its cache of the server's settings, the configuration
//! profile, then the managed settings files with their drop-in folder. A source that is there
//! but cannot be read counts as refusing: a terminal where `claude` would not start is worse
//! than one without the plugin.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// Where Claude Code's managed settings are read from.
pub(super) struct Sources {
    /// The cache of the settings the server manages.
    pub(super) remote: PathBuf,
    /// The configuration profile's files, the person's own first.
    pub(super) mdm: Vec<PathBuf>,
    /// The folder of `managed-settings.json` and `managed-settings.d/`.
    pub(super) folder: PathBuf,
}

/// The managed preferences domain of a configuration profile, as a file.
const PROFILE: &str = "com.anthropic.claudecode.plist";
/// The keys that say how sources combine, which are no policy of their own.
const CONTROL: [&str; 2] = ["managedSourcesBehavior", "wslInheritsWindowsSettings"];

impl Sources {
    /// Where Claude Code on this machine reads them, for an environment that `var` reads.
    pub(super) fn here(var: &dyn Fn(&str) -> Option<String>) -> Self {
        let config = (var("CLAUDE_CONFIG_DIR").filter(|dir| !dir.is_empty()))
            .map(PathBuf::from)
            .or_else(|| var("HOME").map(|home| Path::new(&home).join(".claude")));
        let prefs = Path::new("/Library/Managed Preferences");
        let mut mdm: Vec<PathBuf> = var("USER")
            .filter(|user| !user.is_empty() && !user.contains('/'))
            .map(|user| prefs.join(user).join(PROFILE))
            .into_iter()
            .collect();
        mdm.push(prefs.join(PROFILE));
        let folder = if cfg!(target_os = "macos") {
            "/Library/Application Support/ClaudeCode"
        } else {
            "/etc/claude-code"
        };
        Self {
            remote: config.map_or_else(PathBuf::new, |dir| dir.join("remote-settings.json")),
            mdm,
            folder: PathBuf::from(folder),
        }
    }
}

/// What one source holds.
enum Read {
    Absent,
    Unreadable,
    Settings(Map<String, Value>),
}

/// Whether Claude Code, with the managed settings in `sources`, starts with a plugin folder in
/// its variable. Under "first-wins" (the default) the highest source with a policy key is the
/// policy; under "merge", asked for by the highest source that says either, a refusal in any
/// source is. A policy a program computes (`policyHelper`) is not known, and refuses.
pub(super) fn takes_plugin_folders(sources: &Sources) -> bool {
    let mut found = Vec::new();
    for read in [
        json_file(&sources.remote),
        profile(&sources.mdm),
        files(&sources.folder),
    ] {
        match read {
            Read::Absent => {}
            Read::Unreadable => return false,
            Read::Settings(settings) => found.push(settings),
        }
    }
    let policy = |settings: &Map<String, Value>| {
        (settings.iter()).any(|(key, value)| !CONTROL.contains(&key.as_str()) && !value.is_null())
    };
    let merge = (found.iter())
        .find(|settings| policy(settings) || settings.contains_key(CONTROL[0]))
        .and_then(|settings| settings.get(CONTROL[0]))
        .and_then(Value::as_str)
        == Some("merge");
    let refuses = |settings: &&Map<String, Value>| {
        settings.get("disableSideloadFlags") == Some(&Value::Bool(true))
            || settings.contains_key("policyHelper")
    };
    let mut policies = found.iter().filter(|settings| policy(settings));
    if merge {
        !policies.any(|settings| refuses(&settings))
    } else {
        !policies.next().is_some_and(|settings| refuses(&settings))
    }
}

/// A JSON object in the file at `path`.
fn json_file(path: &Path) -> Read {
    match std::fs::read(path) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Read::Absent,
        Err(_) => Read::Unreadable,
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(Value::Object(settings)) => Read::Settings(settings),
            _ => Read::Unreadable,
        },
    }
}

/// The configuration profile's settings, its files read by the system's `plutil` (a binary
/// property list): one that is there and cannot be read is unreadable. A refusal in either
/// file stands.
fn profile(paths: &[PathBuf]) -> Read {
    let mut merged: Option<Map<String, Value>> = None;
    for path in paths.iter().filter(|path| path.symlink_metadata().is_ok()) {
        let Some(Value::Object(settings)) = converted(path) else {
            return Read::Unreadable;
        };
        let into = merged.get_or_insert_with(Map::new);
        for (key, value) in settings {
            let refusal = value == Value::Bool(true);
            match into.entry(key) {
                serde_json::map::Entry::Vacant(at) => {
                    at.insert(value);
                }
                serde_json::map::Entry::Occupied(mut at) if refusal => {
                    at.insert(value);
                }
                serde_json::map::Entry::Occupied(_) => {}
            }
        }
    }
    merged.map_or(Read::Absent, Read::Settings)
}

/// The property list at `path` as JSON, by the system's `plutil` (a few seconds at most),
/// written to a file of this process's own and read back.
fn converted(path: &Path) -> Option<Value> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let out = std::env::temp_dir().join(format!("llxterm-profile-{}-{n}.json", std::process::id()));
    let mut plutil = std::process::Command::new("plutil");
    plutil
        .args(["-convert", "json", "-o"])
        .arg(&out)
        .arg("--")
        .arg(path);
    let done = crate::fetch::within(plutil, std::time::Duration::from_secs(3), &|| {});
    let json = done.ok().and_then(|()| std::fs::read(&out).ok());
    let _ = std::fs::remove_file(&out);
    serde_json::from_slice(&json?).ok()
}

/// `managed-settings.json`, then each `*.json` of `managed-settings.d/` in the order of their
/// names (hidden ones not read), merged as Claude Code merges them: a later value replaces an
/// earlier one, objects merge key by key, lists combine.
fn files(folder: &Path) -> Read {
    let mut reads = vec![json_file(&folder.join("managed-settings.json"))];
    match std::fs::read_dir(folder.join("managed-settings.d")) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Read::Unreadable,
        Ok(entries) => {
            let mut names: Vec<PathBuf> = (entries.filter_map(Result::ok))
                .map(|entry| entry.path())
                .filter(|path| {
                    let name = path.file_name().map(|name| name.to_string_lossy());
                    name.is_some_and(|name| name.ends_with(".json") && !name.starts_with('.'))
                })
                .collect();
            names.sort();
            reads.extend(names.iter().map(|path| json_file(path)));
        }
    }
    let mut merged: Option<Map<String, Value>> = None;
    for read in reads {
        match read {
            Read::Absent => {}
            Read::Unreadable => return Read::Unreadable,
            Read::Settings(settings) => merge(merged.get_or_insert_with(Map::new), settings),
        }
    }
    merged.map_or(Read::Absent, Read::Settings)
}

/// `from` merged into `into`.
fn merge(into: &mut Map<String, Value>, from: Map<String, Value>) {
    for (key, value) in from {
        match (into.get_mut(&key), value) {
            (Some(Value::Object(old)), Value::Object(new)) => merge(old, new),
            (Some(Value::Array(old)), Value::Array(new)) => {
                for item in new {
                    if !old.contains(&item) {
                        old.push(item);
                    }
                }
            }
            (_, value) => {
                into.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
