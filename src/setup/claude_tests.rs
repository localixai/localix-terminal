//! Tests of `claude`: where Claude Code's marketplace of the name is, and which version of the
//! plugin it has.

use std::path::Path;

use serde_json::json;

use super::{Market, marketplace_in, plugin_in};

#[test]
fn a_marketplace_is_removed_only_when_it_is_listed_at_another_folder() {
    let dir = Path::new("/Applications/LLX Desk.app/Contents/Resources/claude-code");
    let here = json!([{"name": "llx-desk", "path": dir.display().to_string()}]);
    assert_eq!(marketplace_in(&here, dir), Market::Here);
    let elsewhere = json!([{"name": "llx-desk", "path": "/old/claude-code"}]);
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
    let recorded = json!({"id": "llx-desk@llx-desk", "version": "0.1.4", "enabled": true});
    assert_eq!(listed(recorded), Some(("0.1.4".into(), true)));
    // Read from a folder (the app's bundle), the folder's version is what a session loads,
    // whatever was recorded when the plugin was installed.
    let folder = json!({
        "id": "llx-desk@llx-desk", "version": "0.1.4", "folderVersion": "0.1.5",
        "readFromFolder": "/Applications/LLX Desk.app/Contents/Resources/claude-code",
        "enabled": false
    });
    assert_eq!(listed(folder), Some(("0.1.5".into(), false)));
    assert_eq!(
        plugin_in(&json!([{"id": "llx-desk@other", "version": "1"}])),
        None
    );
    assert_eq!(plugin_in(&json!({"plugins": []})), None);
}
