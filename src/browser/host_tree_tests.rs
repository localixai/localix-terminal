//! Tests of the tree's menu: what it offers for a folder, a file a page renders, and any other.

use std::path::Path;

use super::*;

fn titles(path: &str, rel: &str, dir: bool) -> Vec<String> {
    (entries(&groups(Path::new(path), rel, dir), true).iter())
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
            "New File…",
            "New Folder…",
            "-",
            "New Terminal Here",
            "-",
            reveal,
            "Rename…",
            "Move to Trash",
            "-",
            "Insert Path in Terminal",
            "-",
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
            "Rename…",
            "Move to Trash",
            "-",
            "Insert Path in Terminal",
            "-",
            "Copy Path",
            "Copy Relative Path"
        ]
    );
    // A picture written as text opens as the picture; its text is the second way.
    assert_eq!(
        titles("/work/app/art/Mark.SVG", "art/Mark.SVG", false)[..3],
        ["Open in New Tab", "Open as Text", "Open in Default App"]
    );
    assert_eq!(
        titles("/work/app/src/main.rs", "src/main.rs", false)[..2],
        ["Open in New Tab", "Open in Default App"]
    );
    // The tree's own folder has no path under itself, and is neither renamed nor trashed.
    assert_eq!(
        titles("/work/app", "", true),
        [
            "New File…",
            "New Folder…",
            "-",
            "New Terminal Here",
            "-",
            reveal,
            "-",
            "Insert Path in Terminal",
            "-",
            "Copy Path"
        ]
    );
}

#[test]
fn a_path_is_given_only_where_a_terminal_is_in_sight_to_take_it() {
    let off = |terminal: bool| {
        (entries(
            &groups(Path::new("/work/app/a.rs"), "a.rs", false),
            terminal,
        )
        .iter())
        .filter_map(|entry| match entry {
            Entry::Item { label, off, .. } if *off => Some(label.clone()),
            Entry::Item { .. } | Entry::Line { .. } | Entry::Sub { .. } => None,
        })
        .collect::<Vec<_>>()
    };
    assert_eq!(off(true), [""; 0]);
    assert_eq!(off(false), ["Insert Path in Terminal"]);
}
