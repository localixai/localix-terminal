//! Tests of `writes`: what the window tells never waits on the disk, its writes keep their
//! order, and the last of them are done before the app quits.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use super::WRITES;
use crate::browser::Heard;
use crate::desk::tests::{caller, desk_on};
use crate::store::Store;
use crate::testing::Scratch;

const SOON: Duration = Duration::from_secs(5);

/// The store held by another connection's work (the CLI's, a slow disk) until `release` is told.
fn held(store: &Arc<Store>) -> std::sync::mpsc::SyncSender<()> {
    let (release, released) = std::sync::mpsc::sync_channel::<()>(1);
    let (began, holding) = std::sync::mpsc::sync_channel::<()>(1);
    let store = Arc::clone(store);
    std::thread::Builder::new()
        .name("holding".into())
        .spawn(move || {
            store
                .read(|_| {
                    let _ = began.send(());
                    let _ = released.recv_timeout(SOON);
                    Ok(())
                })
                .unwrap();
        })
        .unwrap();
    holding.recv_timeout(SOON).unwrap();
    release
}

#[test]
fn what_the_window_tells_never_waits_on_the_disk_and_is_kept_after() {
    let (scratch, store) = (Scratch::new(), Arc::new(Store::memory().unwrap()));
    let desk = desk_on(Arc::clone(&store));
    let mine = caller(&desk, scratch.path(), None);
    let open =
        json!({"action": "open", "kind": "terminal", "command": "sleep 30", "yield_ms": 200});
    desk.window(&mine.call(), &open).unwrap();
    let group = crate::projects::key(mine.project.id);

    let release = held(&store);
    let began = Instant::now();
    desk.heard("t1", Heard::Closed("closed"));
    desk.heard(&group, Heard::Showing);
    desk.heard("window", Heard::WindowClosed(7));
    assert!(
        began.elapsed() < Duration::from_secs(1),
        "the window's loop waited {:?} on the store",
        began.elapsed()
    );
    // What the agent hears is in memory, at once.
    assert!(
        desk.said(&mine.owner)
            .is_some_and(|said| said.contains("closed"))
    );
    release.send(()).unwrap();

    assert!(desk.writes_done(SOON));
    let open = store
        .read(|db| crate::ledger::left_open(db, mine.project.id))
        .unwrap();
    assert!(
        open.is_empty(),
        "the closed terminal is closed in the ledger"
    );
    desk.shutdown();
}

#[test]
fn a_write_waits_for_room_rather_than_going_ahead_of_older_ones() {
    let desk = desk_on(Arc::new(Store::memory().unwrap()));
    let order = Arc::new(Mutex::new(Vec::new()));
    // The writer is busy with a first write; the queue fills behind it.
    let (go, gate) = std::sync::mpsc::sync_channel::<()>(1);
    desk.write_later(move || {
        let _ = gate.recv_timeout(SOON);
    });
    for n in 0..=WRITES {
        let order = Arc::clone(&order);
        if n == WRITES {
            // The queue is full now: the writer goes on a moment later.
            let go = go.clone();
            std::thread::Builder::new()
                .name("going".into())
                .spawn(move || {
                    crate::backoff(Duration::from_millis(100));
                    let _ = go.send(());
                })
                .unwrap();
        }
        desk.write_later(move || crate::lock(&order).push(n));
    }
    assert!(desk.writes_done(SOON));
    let order = crate::lock(&order).clone();
    assert_eq!(order, (0..=WRITES).collect::<Vec<_>>());
    desk.shutdown();
}

#[test]
fn the_writes_the_window_told_last_are_done_before_the_app_quits() {
    let desk = desk_on(Arc::new(Store::memory().unwrap()));
    let done = Arc::new(AtomicBool::new(false));
    let written = Arc::clone(&done);
    desk.write_later(move || {
        crate::backoff(Duration::from_millis(200));
        written.store(true, Ordering::SeqCst);
    });
    desk.shutdown();
    assert!(done.load(Ordering::SeqCst));
}

#[test]
fn a_page_the_person_closed_leaves_no_lock_behind() {
    let desk = desk_on(Arc::new(Store::memory().unwrap()));
    drop(desk.tab_lock(7));
    desk.heard("w7", Heard::Closed("closed"));
    assert!(!crate::lock(&desk.locks).contains_key(&7));
    desk.shutdown();
}
