//! Tests of `panel`.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use super::*;
use crate::browser::host::Cmd;

/// A panel whose commands are recorded, not sent: what reached the loop, as text.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn recorded(accepted: bool) -> (Panel, Arc<Mutex<Vec<String>>>) {
    let sent = Arc::new(Mutex::new(vec![]));
    let log = Arc::clone(&sent);
    let send = move |command: Cmd| {
        let line = match command {
            Cmd::Paint(id) => format!("paint {id}"),
            Cmd::Unpanel(id) => format!("unpanel {id}"),
            _ => "other".into(),
        };
        log.lock().unwrap().push(line);
        accepted
    };
    (Panel::new(4, Box::new(send)), sent)
}

#[test]
fn paints_before_the_loop_runs_queue_one_command_and_the_loop_takes_all_in_order() {
    let (panel, sent) = recorded(true);
    for frame in ["one", "two", "three"] {
        assert!(panel.paint(frame));
    }
    assert_eq!(*sent.lock().unwrap(), ["paint 4"]);
    // Output is incremental: none of it is skipped.
    assert_eq!(panel.slot.latest().as_deref(), Some("one;two;three;"));
    // Taken: the next paint queues again, and nothing is repeated.
    assert!(panel.paint("four"));
    assert_eq!(*sent.lock().unwrap(), ["paint 4", "paint 4"]);
    assert_eq!(panel.slot.latest().as_deref(), Some("four;"));
    assert_eq!(panel.slot.latest(), None);
}

#[test]
fn frames_dropped_for_size_are_told_to_the_caller_once() {
    let (panel, _) = recorded(true);
    let big = "x".repeat(PENDING_MAX / 2 + 1);
    panel.paint(&big);
    assert!(!panel.dropped(), "all of it is kept");
    panel.paint(&big);
    assert!(panel.dropped(), "the first went to make room");
    assert!(!panel.dropped(), "told once");
    assert_eq!(
        panel.slot.latest().map(|frames| frames.len()),
        Some(big.len() + 1)
    );
}

#[test]
fn dropping_a_panel_closes_its_window_once() {
    let (panel, sent) = recorded(true);
    drop(panel);
    assert_eq!(*sent.lock().unwrap(), ["unpanel 4"]);
}

#[test]
fn a_paint_says_whether_the_window_is_still_open() {
    let (panel, _) = recorded(true);
    assert!(panel.paint("a"));
    panel.slot.alive.store(false, Ordering::SeqCst);
    assert!(!panel.paint("b"), "the person closed it");
    let (gone, _) = recorded(false);
    assert!(!gone.paint("a"), "the loop is gone");
}

#[test]
fn what_a_panels_page_posts_is_keys_typed_or_its_menu_asked_for() {
    assert_eq!(said("kls\r"), Some(Said::Typed("ls\r")));
    assert_eq!(said("k"), Some(Said::Typed("")));
    let Some(Said::Menu(120, 40, items)) = said(r#"m 120 40 [{"label":"Copy"},{"line":true}]"#)
    else {
        panic!("a page's menu is read");
    };
    assert_eq!(items.len(), 2);
    // Nothing else is typed: an old page's bare keys, or a menu said wrong, go nowhere.
    assert_eq!(said("ls"), None);
    assert_eq!(said("m 1 2"), None);
    assert_eq!(said("m 1 2 3"), None);
    assert_eq!(said("m 1 2 [{\"nothing\":1}]"), None);
}

#[test]
fn a_page_says_its_size_when_it_loads_and_when_it_changes() {
    assert_eq!(said("r 132 43"), Some(Said::Sized(Pane::Loaded, 132, 43)));
    assert_eq!(said("s 80 24"), Some(Said::Sized(Pane::Resized, 80, 24)));
    for bad in ["s 80", "s x 24", "s 80 24 1", "r -1 4"] {
        assert_eq!(said(bad), None, "{bad}");
    }
}

#[test]
fn a_page_opens_only_a_web_address_in_a_tab() {
    assert_eq!(
        said("o https://example.com/a?b=1"),
        Some(Said::Open("https://example.com/a?b=1"))
    );
    assert_eq!(
        said("o HTTP://example.com"),
        Some(Said::Open("HTTP://example.com"))
    );
    for bad in [
        "o file:///etc/passwd",
        "o javascript:alert(1)",
        "o ftp://host",
        "o http:/x",
        "o ",
        "o é",
    ] {
        assert_eq!(said(bad), None, "{bad}");
    }
}

#[test]
fn out_of_sight_frames_run_a_while_later_and_at_once_when_shown() {
    let (panel, sent) = recorded(true);
    assert!(!panel.slot.shown(false));
    assert!(panel.paint("W(1)"));
    assert!(panel.paint("W(2)"));
    // Not at once: one paint comes a while later, for all of it.
    assert!(sent.lock().unwrap().is_empty());
    let deadline = std::time::Instant::now() + HIDDEN_EVERY * 20;
    while sent.lock().unwrap().is_empty() {
        assert!(std::time::Instant::now() < deadline, "no paint came");
        std::thread::yield_now();
    }
    assert_eq!(*sent.lock().unwrap(), vec!["paint 4".to_string()]);
    // Back in sight with frames waiting, they run now.
    assert!(panel.paint("W(3)"));
    assert!(panel.slot.shown(true));
    assert_eq!(panel.slot.latest().as_deref(), Some("W(1);W(2);W(3);"));
    // Shown again with nothing waiting, nothing is due.
    assert!(!panel.slot.shown(true));
}
