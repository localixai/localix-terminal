//! Tests of `claude`: where Claude Code's marketplace of the name is, and which version of the
//! plugin it has.

use std::path::Path;

use serde_json::json;

use super::{Market, marketplace_in, on_record, plugin_in};

#[test]
fn a_marketplace_is_removed_only_when_it_is_listed_at_another_folder() {
    let dir = Path::new("/Applications/Localix Terminal.app/Contents/Resources/claude-code");
    let here = json!([{"name": "llxterm", "path": dir.display().to_string()}]);
    assert_eq!(marketplace_in(&here, dir), Market::Here);
    let elsewhere = json!([{"name": "llxterm", "path": "/old/claude-code"}]);
    assert_eq!(marketplace_in(&elsewhere, dir), Market::Elsewhere);
    // Listed cleanly without it: added, nothing to remove.
    let others = json!([{"name": "other", "path": "/x"}]);
    assert_eq!(marketplace_in(&others, dir), Market::Absent);
    assert_eq!(marketplace_in(&json!([]), dir), Market::Absent);
    // Not a list (Claude Code changed its answer): not known.
    assert_eq!(
        marketplace_in(&json!({"marketplaces": []}), dir),
        Market::Unknown
    );
}

#[test]
fn the_plugins_version_is_the_one_claude_code_loads() {
    let listed = |plugin| plugin_in(&json!([{"id": "other@x", "version": "9"}, plugin]));
    let recorded = json!({"id": "llxterm@llxterm", "version": "0.1.4", "enabled": true});
    assert_eq!(listed(recorded), Some(("0.1.4".into(), true)));
    // Read from a folder (the app's bundle), the folder's version is what a session loads,
    // whatever was recorded when the plugin was installed.
    let folder = json!({
        "id": "llxterm@llxterm", "version": "0.1.4", "folderVersion": "0.1.5",
        "readFromFolder": "/Applications/Localix Terminal.app/Contents/Resources/claude-code",
        "enabled": false
    });
    assert_eq!(listed(folder), Some(("0.1.5".into(), false)));
    assert_eq!(
        plugin_in(&json!([{"id": "llxterm@other", "version": "1"}])),
        None
    );
    assert_eq!(plugin_in(&json!({"plugins": []})), None);
}

#[test]
fn the_plugin_is_known_in_place_from_claude_codes_own_records() {
    let dir = Path::new("/Applications/Localix Terminal.app/Contents/Resources/claude-code");
    let installed = |version: &str| json!({"plugins": {"llxterm@llxterm": [{"version": version}]}});
    let markets =
        |path: &str| json!({"llxterm": {"source": {"source": "directory", "path": path}}});
    let settings = |on: bool| json!({"enabledPlugins": {"llxterm@llxterm": on}});
    let here = dir.display().to_string();
    let now = env!("CARGO_PKG_VERSION");
    assert!(on_record(
        &installed(now),
        &markets(&here),
        &settings(true),
        dir
    ));
    // Anything else is for Claude Code itself to say: an older version, a marketplace at
    // another folder, the plugin disabled or not recorded at all.
    assert!(!on_record(
        &installed("0.0.1"),
        &markets(&here),
        &settings(true),
        dir
    ));
    assert!(!on_record(
        &installed(now),
        &markets("/old/claude-code"),
        &settings(true),
        dir
    ));
    assert!(!on_record(
        &installed(now),
        &markets(&here),
        &settings(false),
        dir
    ));
    assert!(!on_record(
        &installed(now),
        &markets(&here),
        &json!({}),
        dir
    ));
    assert!(!on_record(
        &json!({"plugins": {}}),
        &markets(&here),
        &settings(true),
        dir
    ));
}
