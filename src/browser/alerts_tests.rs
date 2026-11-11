//! What waits for the person, said outside the list: which tabs wait, what is told while they
//! are away and taken back, and what is never told late.

use super::{Alerts, Note, read, waits};

/// The center's events as the desk writes them, newest first.
const EVENTS: &str = r#"[
  {"n": 7, "section": "Waiting for You", "urgent": true, "unseen": true, "group": "shop",
   "tab": "t2", "title": "Claude asks: Which database?", "detail": "",
   "buttons": [{"label": "Show", "role": "open"}, {"label": "Dismiss", "role": "dismiss"}]},
  {"n": 5, "section": "Ready for Review", "urgent": true, "unseen": true, "group": "shop",
   "tab": "b1", "title": "Cart", "detail": "fixed it",
   "buttons": [{"label": "Accept", "role": "act"}, {"label": "Open", "role": "open"}]},
  {"n": 4, "section": "Waiting for You", "urgent": true, "unseen": false, "group": "shop",
   "tab": "t2", "title": "Claude finished: older", "detail": "", "buttons": []},
  {"n": 6, "section": "Programs Say", "urgent": true, "unseen": true, "group": "shop",
   "tab": "t3", "title": "make", "detail": "Build failed", "words": "Build failed",
   "buttons": [{"label": "Show", "role": "open"}]},
  {"n": 3, "section": "Programs Finished", "urgent": false, "unseen": true, "group": "shop",
   "tab": "t1", "title": "cargo test", "detail": "exited with 0", "buttons": []},
  {"n": 2, "section": "Waiting for You", "urgent": true, "unseen": true, "group": "shop",
   "tab": "", "title": "Claude finished: no terminal", "detail": "", "buttons": []}
]"#;

#[test]
fn a_tab_waits_with_the_words_of_what_wants_the_person_there_most() {
    let waiting = waits(&read(EVENTS));
    let mut tabs: Vec<_> = (waiting.iter())
        .map(|(tab, words)| (&**tab, &**words))
        .collect();
    tabs.sort_unstable();
    // What ended well marks nothing, and an event of no tab has no tab to mark.
    assert_eq!(
        tabs,
        [
            ("b1", "Cart"),
            ("t2", "Claude asks: Which database?"),
            ("t3", "Build failed")
        ]
    );
    assert!(waits(&read("")).is_empty() && waits(&read("not json")).is_empty());
}

/// Alerts of an app that has begun: its list heard once, with nothing in it.
fn begun() -> Alerts {
    let mut alerts = Alerts::default();
    let _ = alerts.heard(&[], false);
    alerts
}

#[test]
fn what_waited_before_the_app_was_started_is_counted_and_not_told_again() {
    // The list as the journal kept it, heard first: the person was told of it in the run
    // before, so the Dock counts it and no notice comes; what comes after is news.
    let mut alerts = Alerts::default();
    let kept = EVENTS.replace(r#"{"n": 7,"#, r#"{"n": 70,"#);
    let first = alerts.heard(&read(&kept), false);
    assert!(first.post.is_empty());
    assert_eq!(first.badge, 4);
    let after = alerts.heard(&read(EVENTS), false);
    let told: Vec<u64> = after.post.iter().map(|note| note.id).collect();
    assert_eq!(told, [7]);
}

#[test]
fn what_is_new_while_the_person_is_away_is_told_once_and_taken_back_when_it_goes() {
    let mut alerts = begun();
    let events = read(EVENTS);
    let change = alerts.heard(&events, false);
    // Each new thing that wants them, the list's first last, so it lies on top; its click shows its
    // tab where it has one to show, else opens the list. Accepting a card is never a click's.
    assert_eq!(
        change.post,
        [
            Note {
                id: 2,
                title: "Claude finished: no terminal".into(),
                body: "Waiting for You · shop".into(),
                click: "events open".into(),
            },
            Note {
                id: 6,
                title: "Build failed".into(),
                body: "Programs Say · make · shop".into(),
                click: "event 6 0".into(),
            },
            Note {
                id: 5,
                title: "Cart".into(),
                body: "Ready for Review · fixed it · shop".into(),
                click: "event 5 1".into(),
            },
            Note {
                id: 7,
                title: "Claude asks: Which database?".into(),
                body: "Waiting for You · shop".into(),
                click: "event 7 0".into(),
            },
        ]
    );
    assert_eq!((change.badge, change.withdraw.len()), (4, 0));

    // Heard again, nothing is told twice.
    let again = alerts.heard(&events, false);
    assert!(again.post.is_empty() && again.withdraw.is_empty());
    assert_eq!(again.badge, 4);

    // The session went on and the list was seen: what was told is taken back, the badge goes.
    let seen = EVENTS.replace(r#""unseen": true"#, r#""unseen": false"#);
    let change = alerts.heard(&read(&seen), true);
    let mut gone = change.withdraw;
    gone.sort_unstable();
    assert_eq!((gone, change.badge), (vec![2, 5, 6, 7], 0));
    assert!(change.post.is_empty());
}

#[test]
fn what_comes_while_the_person_is_in_the_app_is_never_told_later() {
    let mut alerts = begun();
    let events = read(EVENTS);
    // They are here: the bell and the tab say it, and the Dock counts it.
    let here = alerts.heard(&events, true);
    assert!(here.post.is_empty());
    assert_eq!(here.badge, 4);
    // They left: what they were here for is not news.
    assert!(alerts.heard(&events, false).post.is_empty());
}
