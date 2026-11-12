//! A kind that draws itself has a key, a picture, a row of "+" and a shortcut without the
//! window naming it.

use crate::browser::strip::{Key, MADE, made, page};
use crate::kind::testing::Echo;

#[test]
fn its_tabs_keys_are_its_letter_and_read_back_as_its_panels() {
    let _echo = Echo::installed();
    let panel = made(0, 3).unwrap();
    assert!(panel > MADE);
    let key = Key::Term(panel);
    assert_eq!(key.name(), "x3");
    assert_eq!(key.kind(), "echo");
    assert_eq!(Key::parse("x3"), Some(key));
    assert_eq!(Key::parse("x0"), None);
    assert_eq!(Key::parse("q1"), None);
}

#[test]
fn the_keys_there_always_were_are_what_they_were() {
    let _echo = Echo::installed();
    for key in ["w1", "t7", "s1", "b2", "f9"] {
        assert_eq!(Key::parse(key).map(Key::name).as_deref(), Some(key));
    }
}

#[test]
fn the_page_is_given_the_kinds_as_data() {
    assert!(page("{}").contains(r#"id="kinds">[]<"#));
    let _echo = Echo::installed();
    let page = page("{}");
    assert!(
        page.contains(r#"id="kinds">[{"glyph":"M3 8h10","holds":"What it was told","id":"echo","key":"key new echo","title":"Echo"}]<"#),
        "the kinds are in the page"
    );
}

#[test]
fn its_command_is_a_shortcut_after_the_boards_and_its_key_asks_for_its_tab() {
    use crate::browser::shortcuts::every;
    let before = every().len();
    let _echo = Echo::installed();
    let all = every();
    assert_eq!(all.len(), before + 1);
    let board = all.iter().position(|s| s.id == "board").unwrap();
    assert_eq!(all[board + 1].id, "echo");
    assert_eq!(all[board + 1].does, "key new echo");
    assert_eq!(all[board + 1].keys, "cmd+alt+e");
    assert_eq!(all[board + 1].group, "Tabs");
}

#[test]
fn its_picture_in_a_menu_is_made_of_its_glyph() {
    let _echo = Echo::installed();
    let picture = crate::browser::icons::of_kind("echo");
    assert!(picture.contains(r#"<path d="M3 8h10"/>"#), "{picture}");
    assert!(picture.starts_with("<svg"));
}
