//! Tests of `claude`: where Claude Code's marketplace of the name is, and which version of the
//! plugin it has.

use std::path::Path;

use serde_json::json;

use super::{Former, Market, former_in, holds_former, marketplace_in, on_record, plugin_in};

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

#[test]
fn llx_desks_plugin_and_marketplace_are_found_in_claude_codes_records() {
    let old = "/Applications/LLX Desk.app/Contents/Resources/claude-code";
    let none = json!({});
    // As this Mac's settings have them: the marketplace a folder in 0.1.6's bundle.
    let settings = json!({
        "enabledPlugins": {"llx-desk@llx-desk": true, "llxterm@llxterm": true},
        "extraKnownMarketplaces": {"llx-desk": {"source": {"source": "directory", "path": old}}}
    });
    assert_eq!(
        former_in(&none, &none, &settings),
        Former::Recorded(Some(old.into()))
    );
    let markets = json!({"llx-desk": {"source": {"source": "directory", "path": "/elsewhere"}}});
    assert_eq!(
        former_in(&none, &markets, &none),
        Former::Recorded(Some("/elsewhere".into()))
    );
    // The plugin alone, its marketplace gone already.
    let installed = json!({"plugins": {"llx-desk@llx-desk": [{"version": "0.1.6"}]}});
    assert_eq!(former_in(&installed, &none, &none), Former::Recorded(None));
    // This app's own are not LLX Desk's.
    let new = json!({"enabledPlugins": {"llxterm@llxterm": true}});
    assert_eq!(former_in(&none, &none, &new), Former::Absent);
}

#[test]
fn a_marketplace_folder_holds_llx_desks_only_while_its_manifest_names_it() {
    let scratch = crate::testing::Scratch::new();
    let dir = scratch.path();
    assert!(!holds_former(dir));
    let manifest = ".claude-plugin/marketplace.json";
    scratch.file(manifest, r#"{"name": "llx-desk", "plugins": []}"#);
    assert!(holds_former(dir));
    // 0.1.6's bundle with this app in it: the folder is there, and holds this app's.
    scratch.file(manifest, r#"{"name": "llxterm", "plugins": []}"#);
    assert!(!holds_former(dir));
}
