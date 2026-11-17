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
    // None of it is skipped.
    assert_eq!(panel.slot.latest().as_deref(), Some("one;two;three;"));
    // Taken: the next paint queues again, and nothing is repeated.
    assert!(panel.paint("four"));
    assert_eq!(*sent.lock().unwrap(), ["paint 4", "paint 4"]);
    assert_eq!(panel.slot.latest().as_deref(), Some("four;"));
    assert_eq!(panel.slot.latest(), None);
}

#[test]
fn frames_past_the_most_make_room_for_the_newest() {
    let (panel, _) = recorded(true);
    let big = "x".repeat(PENDING_MAX / 2 + 1);
    panel.paint(&big);
    panel.paint(&big);
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
fn a_page_says_when_it_has_loaded() {
    assert_eq!(said("r 0 0"), Some(Said::Loaded));
    assert_eq!(said("r"), Some(Said::Loaded));
    for bad in ["s 80 24", "c aGk", "rr"] {
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

/// Where the person looks, as a check gives it: no window the system made key is needed.
struct Held {
    shown: u32,
    beside: Option<u32>,
    key_window: bool,
}

impl Keys for Held {
    fn in_sight(&self, id: u32) -> bool {
        self.shown == id || self.beside == Some(id)
    }
    fn key_window(&self) -> bool {
        self.key_window
    }
    /// The terminal shown has the keys.
    fn typing(&self, id: u32) -> bool {
        self.shown == id
    }
}

#[test]
fn a_programs_copy_reaches_the_pasteboard_only_from_the_terminal_the_person_types_in() {
    let looking = Held {
        shown: 3,
        beside: Some(4),
        key_window: true,
    };
    let copied = |keys: &Held, id: u32| {
        let mut written = None;
        let wrote = copy_where(keys, id, "secret", |text| written = Some(text.to_owned()));
        assert_eq!(wrote, written.is_some());
        written
    };
    assert_eq!(copied(&looking, 3).as_deref(), Some("secret"));
    assert_eq!(copied(&looking, 4), None, "in sight, but not typed in");
    assert_eq!(copied(&looking, 5), None, "out of sight");
    let away = Held {
        key_window: false,
        ..looking
    };
    assert_eq!(copied(&away, 3), None, "another window or app has the keys");
}

#[test]
fn a_command_that_ended_is_news_only_when_it_ran_long_and_the_person_was_not_looking() {
    use std::time::Duration;
    let (minute, after) = (Duration::from_secs(60), Some(Duration::from_secs(30)));
    let looking = Held {
        shown: 3,
        beside: Some(4),
        key_window: true,
    };
    let news = |keys: Option<&Held>, id: u32, took: Duration, after, active: bool| {
        unattended(keys.map(|keys| keys as &dyn Keys), id, took, after, active)
    };
    // In sight in the key window of the app in use: seen, in either pane of the split.
    assert!(!news(Some(&looking), 3, minute, after, true));
    assert!(!news(Some(&looking), 4, minute, after, true));
    // Shorter than the setting, or the setting off: never news.
    assert!(!news(
        Some(&looking),
        5,
        Duration::from_secs(29),
        after,
        true
    ));
    assert!(!news(Some(&looking), 5, minute, None, true));
    // Another tab shown, the window not the key one, the person in another app, no window.
    assert!(news(Some(&looking), 5, minute, after, true), "out of sight");
    let behind = Held {
        key_window: false,
        ..looking
    };
    assert!(
        news(Some(&behind), 3, minute, after, true),
        "another window is key"
    );
    assert!(
        news(Some(&looking), 3, minute, after, false),
        "another app is in use"
    );
    assert!(news(None, 3, minute, after, true), "no window shows it");
    // Exactly the setting is long enough.
    assert!(news(None, 3, Duration::from_secs(30), after, true));
}
