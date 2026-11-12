//! The Store's note of what it installed is kept across runs and says what is behind.

use super::*;
use crate::testing::Scratch;

#[test]
fn what_was_installed_is_kept_and_what_was_removed_is_forgotten() {
    let scratch = Scratch::new();
    let file = scratch.path().join("installed.json");
    let mut note = Installed::open(&file);
    note.set("skills/pdf", "claude", Some("abc"));
    note.set("skills/pdf", "codex", Some("abc"));
    note.set("skills/pdf", "codex", None);
    let again = Installed::open(&file);
    assert_eq!(again.ids(), ["skills/pdf"]);
    let agents = again.of("skills/pdf").unwrap();
    assert_eq!(agents.get("claude").map(String::as_str), Some("abc"));
    assert!(!agents.contains_key("codex"));
    let mut again = again;
    again.set("skills/pdf", "claude", None);
    assert!(Installed::open(&file).ids().is_empty());
}

#[test]
fn a_listing_is_behind_when_an_agent_has_another_version_of_it() {
    let mut note = Installed::default();
    note.set("a/b", "claude", Some("1"));
    assert!(note.behind("a/b", "2"));
    assert!(!note.behind("a/b", "1"));
    assert!(
        !note.behind("a/b", ""),
        "a listing that pins nothing is never behind"
    );
    note.set("a/c", "claude", Some(""));
    assert!(
        !note.behind("a/c", "2"),
        "nor one installed at no known version"
    );
    assert!(!note.behind("a/none", "2"));
}

#[test]
fn a_file_that_does_not_read_is_an_empty_note() {
    let scratch = Scratch::new();
    let file = scratch.path().join("installed.json");
    std::fs::write(&file, "{ not json").unwrap();
    assert!(Installed::open(&file).ids().is_empty());
}
