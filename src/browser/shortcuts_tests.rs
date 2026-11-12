//! Tests of `shortcuts`.

use std::collections::BTreeMap;

use super::catalog::OF_PAGE;
use super::*;

#[test]
fn a_chord_reads_from_words_and_back_and_shows_as_the_system_draws_it() {
    let chord = Chord::parse("Shift+Cmd+]").unwrap();
    assert!(chord.cmd && chord.shift && !chord.alt && !chord.ctrl);
    assert_eq!(chord.text(), "shift+cmd+]");
    assert_eq!(chord.shown(), "\u{21e7}\u{2318}]");
    assert_eq!(chord.menu_key(), ("}".to_string(), false));
    let letter = Chord::parse("cmd+shift+t").unwrap();
    assert_eq!(letter.shown(), "\u{21e7}\u{2318}T");
    assert_eq!(letter.menu_key(), ("t".to_string(), true));
    assert_eq!(
        Chord::parse("ctrl+cmd+s").unwrap().shown(),
        "\u{2303}\u{2318}S"
    );
    assert_eq!(Chord::parse("cmd+return").unwrap().key, "enter");
    assert_eq!(Chord::parse("cmd++").unwrap().key, "+");
    // Typing is no shortcut, and a word that is no key no chord.
    for text in ["t", "shift+t", "cmd+hyper+t", "cmd+tt", ""] {
        assert_eq!(Chord::parse(text), None, "{text}");
    }
}

#[test]
fn every_default_is_a_chord_and_no_two_share_one() {
    let resolved = resolved(&BTreeMap::new());
    for (shortcut, chord) in &resolved {
        assert!(chord.is_some(), "{}", shortcut.id);
        assert!(
            shortcut
                .does
                .split(' ')
                .next()
                .is_some_and(|verb| !verb.is_empty())
        );
    }
    assert_eq!(clashes(&resolved), Vec::<&str>::new());
    let ids: std::collections::HashSet<_> = every().iter().map(|s| s.id).collect();
    assert_eq!(ids.len(), every().len());
}

#[test]
fn the_persons_chord_wins_none_takes_it_off_and_a_shared_one_is_named() {
    let changed = BTreeMap::from([
        ("reload".to_string(), Some("cmd+shift+r".to_string())),
        ("history".to_string(), None),
        ("send".to_string(), Some("cmd+l".to_string())),
        ("gone".to_string(), Some("cmd+g".to_string())),
    ]);
    let resolved = resolved(&changed);
    let chord = |id: &str| {
        (resolved.iter().find(|(s, _)| s.id == id))
            .and_then(|(_, chord)| chord.as_ref().map(Chord::text))
    };
    assert_eq!(chord("reload").as_deref(), Some("shift+cmd+r"));
    assert_eq!(chord("history"), None);
    assert_eq!(clashes(&resolved), ["address", "send"]);
}

#[test]
fn a_row_of_places_is_one_chord_for_each_digit_and_meets_a_single_one() {
    let row = Chord::parse("cmd+digit").unwrap();
    assert_eq!(row.shown(), "\u{2318}1\u{2026}8");
    let each = row.each();
    assert_eq!(each.len(), 8);
    assert_eq!(each[2].0.text(), "cmd+3");
    assert_eq!(each[2].1, Some(3));
    assert!(row.meets(&Chord::parse("cmd+5").unwrap()));
    assert!(!row.meets(&Chord::parse("cmd+9").unwrap()));
    assert!(!row.meets(&Chord::parse("ctrl+5").unwrap()));
    // Taking ⌘5 for another shortcut names both.
    let changed = BTreeMap::from([("reload".to_string(), Some("cmd+5".to_string()))]);
    assert_eq!(clashes(&resolved(&changed)), ["tab-place", "reload"]);
}

#[test]
fn each_group_is_one_menu_listed_once_with_the_apps_own_last() {
    let groups = groups();
    let mut unique = groups.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        groups.len(),
        "a group's shortcuts are apart: {groups:?}"
    );
    assert_eq!(groups.last(), Some(&"LLX Desk"));
    assert!(
        every()
            .iter()
            .any(|s| s.does == "quit" && s.keys == "cmd+q")
    );
}

#[test]
fn what_needs_a_page_is_in_the_catalog() {
    for does in OF_PAGE {
        assert!(every().iter().any(|s| s.does == does), "{does}");
    }
    assert!(of_page("reload") && !of_page("key address") && !of_page("send"));
}

#[test]
fn one_rule_says_what_a_shortcuts_keys_are_for_the_buttons_and_the_menus() {
    let page = every().into_iter().find(|s| s.id == "new-page").unwrap();
    let text =
        |changed: &BTreeMap<String, Option<String>>| chord_of(changed, page).map(|c| c.text());
    assert_eq!(text(&BTreeMap::new()).as_deref(), Some("cmd+t"));
    let set = BTreeMap::from([("new-page".to_string(), Some("cmd+shift+n".to_string()))]);
    assert_eq!(text(&set).as_deref(), Some("shift+cmd+n"));
    let off = BTreeMap::from([("new-page".to_string(), None)]);
    assert_eq!(text(&off), None);
    // A keys the person's file holds for a shortcut the catalog lacks is nobody's.
    assert_eq!(keys_of("gone-for-good"), None);
}
