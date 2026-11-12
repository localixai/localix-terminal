//! Tests of the tree's menu: what it offers for a folder, a file a page renders, and any other.

use std::path::Path;

use super::*;

fn titles(path: &str, rel: &str, dir: bool) -> Vec<String> {
    (entries(&groups(Path::new(path), rel, dir)).iter())
        .map(|entry| match entry {
            Entry::Item { label, .. } => label.clone(),
            Entry::Line { .. } => "-".to_string(),
            Entry::Sub { label, .. } => format!("{label} >"),
        })
        .collect()
}

#[test]
fn a_menu_offers_what_its_entry_can_do() {
    let reveal = platform::REVEAL;
    assert_eq!(
        titles("/work/app/src", "src", true),
        [
            "New Terminal Here",
            "-",
            reveal,
            "Copy Path",
            "Copy Relative Path"
        ]
    );
    assert_eq!(
        titles("/work/app/site/Index.HTML", "site/Index.HTML", false),
        [
            "Open in New Tab",
            "Open as Page",
            "Open in Default App",
            "-",
            reveal,
            "Copy Path",
            "Copy Relative Path"
        ]
    );
    assert_eq!(
        titles("/work/app/src/main.rs", "src/main.rs", false),
        [
            "Open in New Tab",
            "Open in Default App",
            "-",
            reveal,
            "Copy Path",
            "Copy Relative Path"
        ]
    );
    // The tree's own folder has no path under itself.
    assert_eq!(
        titles("/work/app", "", true),
        ["New Terminal Here", "-", reveal, "Copy Path"]
    );
}
