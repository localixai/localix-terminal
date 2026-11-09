//! Tests of `deck_said`.

use super::*;

#[test]
fn the_same_script_again_is_no_news() {
    let mut said = Said::default();
    // The history menu, the Event Center and the toolbar's pages seen, as a refresh tells them.
    for (kind, script) in [
        ("menu", "M(null)"),
        ("center", "V([], false)"),
        ("visited", "VI([])"),
    ] {
        assert!(said.news(0, kind, script, ()), "{kind}");
        assert!(!said.news(0, kind, script, ()), "{kind} again");
    }
    assert!(said.news(0, "center", "V([{\"id\":1}], true)", ()));
    assert!(!said.news(0, "menu", "M(null)", ()));
}

#[test]
fn a_page_loaded_again_is_told_all_again() {
    let mut said = Said::default();
    assert!(said.news(0, "menu", "M(null)", ()));
    assert!(said.news(1, "menu", "M(null)", ()));
    assert!(!said.news(1, "menu", "M(null)", ()));
}

#[test]
fn the_shown_page_is_told_again_once_it_loaded_or_another_tab_is_shown() {
    let mut said = Said::default();
    let comment =
        "window.__dloop_call && window.__dloop_call(0, 'comment', { on: false, sent: 0 })";
    // Tab 1, its page loaded once (`finished` 4).
    assert!(said.news(0, "comment", comment, (1_u32, 4_u64)));
    assert!(!said.news(0, "comment", comment, (1_u32, 4_u64)));
    // Its page loaded again: the new document knows nothing yet.
    assert!(said.news(0, "comment", comment, (1_u32, 9_u64)));
    // Another tab shown.
    assert!(said.news(0, "comment", comment, (2_u32, 9_u64)));
}

#[test]
fn a_part_drawn_over_is_told_again() {
    let mut said = Said::default();
    assert!(said.news(0, "menu", "M(null)", ()));
    said.forget(&"menu");
    assert!(said.news(0, "menu", "M(null)", ()));
}

#[test]
fn a_click_draws_the_window_once() {
    let held = Held::default();
    assert!(held.now(), "not held: drawn at once");
    // A click: the act refreshes, the settle acts and refreshes again; then the end draws.
    held.hold();
    let mut drawn = (0..2).filter(|_| held.now()).count();
    held.hold();
    drawn += (0..2).filter(|_| held.now()).count();
    held.release();
    drawn += usize::from(held.now());
    assert_eq!(drawn, 1);
}
