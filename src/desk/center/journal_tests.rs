//! Journal persistence, stale run-local events and reused ids.

use super::*;

#[test]
fn the_journal_keeps_what_the_person_saw_across_a_restart_and_what_settled_as_history() {
    let store = Arc::new(crate::store::Store::memory().unwrap());
    let scratch = Scratch::new();
    let first = desk_on(Arc::clone(&store));
    let mine = caller(&first, scratch.path(), None);
    let me = mine.call();
    on_board(&first, &me, json!({"op": "add", "title": "Cart"}));
    on_board(&first, &me, json!({"op": "take", "card": "c1"}));
    on_board(
        &first,
        &me,
        json!({"op": "review", "card": "c1", "note": "fixed; checked"}),
    );
    assert!(shown(&first)[0].unseen);
    first.center_seen();
    first.shutdown();
    assert!(first.feed.ended(Duration::from_secs(5)));

    // The desk again, on the same store: the card still waits, and is still seen.
    let again = desk_on(Arc::clone(&store));
    on_board(&again, &me, json!({"op": "look"}));
    let events = shown(&again);
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(!events[0].unseen, "seen before the restart");

    // Accepted: it settles, and the journal keeps it.
    let posted = json!({"op": "accept", "card": "c1"}).to_string();
    again.person_on_board(mine.project.id, &posted);
    assert!(shown(&again).is_empty());
    let settled: u64 = store
        .read(|db| {
            db.query_row(
                "SELECT COUNT(*) FROM center_events WHERE settled IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|why| why.to_string())
        })
        .unwrap();
    assert_eq!(settled, 1);
    again.shutdown();
}

#[test]
fn an_id_seen_again_after_it_settled_or_in_another_project_is_a_new_event() {
    let store = crate::store::Store::memory().unwrap();
    let (one, two) = (Scratch::new(), Scratch::new());
    let (a, b) = store
        .write(|tx| {
            let a = crate::projects::find_or_make(tx, one.path(), 1)?;
            let b = crate::projects::find_or_make(tx, two.path(), 1)?;
            Ok((a, b))
        })
        .unwrap();
    let ended = |project: &crate::projects::Project| {
        let group = crate::projects::key(project.id);
        Event::new("ended:t3".into(), Kind::Finished, &group, "t3")
    };
    let record =
        |events: &[Event], now: u64| store.write(|tx| journal::record(tx, events, now)).unwrap();
    record(&[ended(&a)], 10);
    let ids = vec!["ended:t3".to_string()];
    store
        .write(|tx| journal::mark(tx, &ids, journal::Mark::Dismissed))
        .unwrap();
    // While it is open, the mark stays.
    assert!(record(&[ended(&a)], 11)["ended:t3"].dismissed);
    // Of another project (ids count afresh each run), it is new.
    assert!(!record(&[ended(&b)], 12)["ended:t3"].dismissed);
    store
        .write(|tx| journal::mark(tx, &ids, journal::Mark::Dismissed))
        .unwrap();
    // Settled (gone), then seen again: new as well.
    record(&[], 13);
    let again = record(&[ended(&b)], 14);
    assert!(!again["ended:t3"].dismissed);
    assert_eq!(again["ended:t3"].first, 14);
}

#[test]
fn what_only_the_last_run_held_is_settled_at_start_and_a_boards_news_stays() {
    let store = crate::store::Store::memory().unwrap();
    let scratch = Scratch::new();
    let a = store
        .write(|tx| crate::projects::find_or_make(tx, scratch.path(), 1))
        .unwrap();
    let group = crate::projects::key(a.id);
    let ended = || Event::new("ended:t1".into(), Kind::Finished, &group, "t1");
    let review = || Event::new("review:1:c1:5".into(), Kind::Review, &group, "b1");
    let record =
        |events: &[Event], now: u64| store.write(|tx| journal::record(tx, events, now)).unwrap();
    record(&[ended(), review()], 10);
    let ids = vec!["ended:t1".to_string(), "review:1:c1:5".to_string()];
    store
        .write(|tx| journal::mark(tx, &ids, journal::Mark::Dismissed))
        .unwrap();
    // The app starts again: a program that ended before is past, a card waiting is still there.
    store.write(|tx| journal::settle_run_local(tx, 20)).unwrap();
    let kept = record(&[ended(), review()], 21);
    assert!(!kept["ended:t1"].dismissed);
    assert!(kept["review:1:c1:5"].dismissed);
}
