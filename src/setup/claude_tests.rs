//! Tests of `claude`: where Claude Code's marketplace of the name is.

use std::path::Path;

use serde_json::json;

use super::{Market, marketplace_in};

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
