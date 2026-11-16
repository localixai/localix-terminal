//! Tests of `marks`: what a tab carries to the strip's page of what waits in it, of its
//! program at work, and of its tab's zoom.

use super::super::{Item, Key, Marks, Working, script};
use super::none;

fn listed(marks: Marks) -> String {
    let items = [Item {
        key: Key::Term(4),
        title: "session 4 · shell".into(),
        detail: "okak@Mac:~".into(),
        group: "proj".into(),
        agent: Some("claude"),
        marks,
        of: None,
    }];
    script(&items, Key::Web(1), Some(true), &none())
}

#[test]
fn a_tab_that_waits_for_the_person_carries_the_words_of_what_waits() {
    let waits = Some("Claude Code: approve Bash: npm test".to_string());
    let drawn = listed(Marks {
        waits,
        ..Marks::default()
    });
    assert!(
        drawn.contains(r#""waits":"Claude Code: approve Bash: npm test""#),
        "{drawn}"
    );
    // A tab nothing waits in says nothing of it.
    let drawn = listed(Marks::default());
    assert!(
        !drawn.contains("waits") && !drawn.contains("working"),
        "{drawn}"
    );
}

#[test]
fn a_terminal_whose_program_is_at_work_carries_how_far_if_it_says() {
    let at = |progress| Marks {
        working: Some(Working { progress }),
        ..Marks::default()
    };
    let drawn = listed(at(Some(40)));
    assert!(drawn.contains(r#""working":{"progress":40}"#), "{drawn}");
    let drawn = listed(at(None));
    assert!(drawn.contains(r#""working":{"progress":null}"#), "{drawn}");
}

#[test]
fn a_pane_its_tab_is_zoomed_on_carries_how_many_panes_are_put_by() {
    let drawn = listed(Marks {
        zoomed: Some(2),
        ..Marks::default()
    });
    assert!(drawn.contains(r#""zoomed":2"#), "{drawn}");
    assert!(!listed(Marks::default()).contains("zoomed"));
}
