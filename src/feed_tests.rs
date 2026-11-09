//! Tests of `feed`: order, a cursor per consumer, retries, notices set aside, and a gap.

use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

const SOON: Duration = Duration::from_secs(5);

/// A consumer that writes down what it handled, fails as `fails` says, and counts reconciles.
struct Seen {
    name: &'static str,
    handled: Arc<Mutex<Vec<u64>>>,
    reconciled: Arc<AtomicU32>,
    fails: Box<dyn FnMut(&Notice) -> bool + Send>,
    gate: Option<Arc<(Mutex<bool>, Condvar)>>,
}

impl Seen {
    fn new(name: &'static str) -> (Self, Arc<Mutex<Vec<u64>>>, Arc<AtomicU32>) {
        let (handled, reconciled) = (Arc::default(), Arc::default());
        let seen = Self {
            name,
            handled: Arc::clone(&handled),
            reconciled: Arc::clone(&reconciled),
            fails: Box::new(|_| false),
            gate: None,
        };
        (seen, handled, reconciled)
    }
}

impl Consumer for Seen {
    fn name(&self) -> &'static str {
        self.name
    }

    fn kind(&self) -> Kind {
        Kind::Projector
    }

    fn handle(&mut self, notice: &Notice) -> Result<(), String> {
        if let Some(gate) = &self.gate {
            let (open, opened) = &**gate;
            let mut open = crate::lock(open);
            while !*open {
                open = opened
                    .wait(open)
                    .map_err(|_poisoned| "poisoned".to_string())?;
            }
        }
        if (self.fails)(notice) {
            return Err(format!("no {}", notice.seq));
        }
        crate::lock(&self.handled).push(notice.seq);
        Ok(())
    }

    fn reconcile(&mut self) {
        self.reconciled.fetch_add(1, Ordering::SeqCst);
    }
}

/// A board's notice: the project's id is the number in `id` ("c3": 3).
fn card(id: &str) -> Entity {
    let project = id.trim_start_matches('c').parse().unwrap_or_default();
    Entity::Board { project }
}

#[test]
fn each_consumer_reads_every_notice_in_order_at_its_own_pace() {
    let feed = Feed::new(Arc::new(Store::memory().unwrap()), Duration::from_millis(1));
    let (fast, fast_seen, reconciled) = Seen::new("fast");
    let (mut slow, slow_seen, _) = Seen::new("slow");
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    slow.gate = Some(Arc::clone(&gate));
    feed.consume(Box::new(fast)).unwrap();
    feed.consume(Box::new(slow)).unwrap();
    for id in ["c1", "c2", "c3"] {
        feed.record(&card(id), Op::Upserted);
    }
    // The slow one holds its first notice: the fast one is not held with it.
    let deadline = Instant::now() + SOON;
    while fast_seen.lock().unwrap().len() < 3 {
        assert!(
            Instant::now() < deadline,
            "the fast consumer waited on the slow one"
        );
        std::thread::yield_now();
    }
    // Notices are numbered from 1, as the store numbers its rows.
    assert_eq!(*fast_seen.lock().unwrap(), [1, 2, 3]);
    assert!(slow_seen.lock().unwrap().is_empty());
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    assert!(feed.settled(SOON));
    assert_eq!(*slow_seen.lock().unwrap(), [1, 2, 3]);
    // A new consumer reconciles once, then reads from now on.
    assert_eq!(reconciled.load(Ordering::SeqCst), 1);
    let (late, late_seen, _) = Seen::new("late");
    feed.consume(Box::new(late)).unwrap();
    feed.record(&card("c4"), Op::Deleted);
    assert!(feed.settled(SOON));
    assert_eq!(*late_seen.lock().unwrap(), [4]);
    feed.stop();
}

#[test]
fn a_failed_notice_is_tried_again_and_set_aside_after_a_few_attempts() {
    let feed = Feed::new(Arc::new(Store::memory().unwrap()), Duration::from_millis(1));
    let (mut flaky, handled, reconciled) = Seen::new("flaky");
    let tries = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&tries);
    // The first notice fails twice and then goes; the second never does.
    flaky.fails = Box::new(move |notice| match notice.seq {
        1 => counted.fetch_add(1, Ordering::SeqCst) < 2,
        _ => true,
    });
    feed.consume(Box::new(flaky)).unwrap();
    feed.record(&card("c1"), Op::Upserted);
    feed.record(&card("c2"), Op::Upserted);
    assert!(feed.settled(SOON));
    assert_eq!(
        *handled.lock().unwrap(),
        [1],
        "the first went on its third try"
    );
    let health = &feed.health()[0];
    assert_eq!(
        (
            health.name,
            health.cursor,
            health.dead_letters,
            health.attempts
        ),
        ("flaky", 3, 1, 0)
    );
    assert_eq!(health.last_error.as_deref(), Some("no 2"));
    // Once at the start, once for the notice set aside.
    assert_eq!(reconciled.load(Ordering::SeqCst), 2);
    feed.stop();
}

#[test]
fn a_consumer_behind_what_the_feed_keeps_reconciles_and_goes_on() {
    let feed = Feed::new(Arc::new(Store::memory().unwrap()), Duration::from_millis(1));
    let (mut held, handled, reconciled) = Seen::new("held");
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    held.gate = Some(Arc::clone(&gate));
    feed.consume(Box::new(held)).unwrap();
    for n in 0..=KEPT {
        feed.record(&card(&format!("c{n}")), Op::Upserted);
    }
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    assert!(feed.settled(SOON));
    assert!(reconciled.load(Ordering::SeqCst) >= 1);
    let handled = handled.lock().unwrap();
    assert_eq!(handled.last(), Some(&(KEPT + 1)));
    feed.stop();
}

#[test]
fn a_stopped_feed_takes_no_more_notices() {
    let feed = Feed::new(Arc::new(Store::memory().unwrap()), Duration::from_millis(1));
    let (seen, handled, _) = Seen::new("seen");
    feed.consume(Box::new(seen)).unwrap();
    feed.stop();
    feed.record(&card("c1"), Op::Upserted);
    assert!(feed.settled(SOON));
    assert!(handled.lock().unwrap().is_empty());
}

#[test]
fn after_a_restart_each_consumer_goes_on_from_where_it_was() {
    let scratch = crate::testing::Scratch::new();
    let path = scratch.path().join("desk.db");
    {
        let feed = Feed::new(
            Arc::new(Store::open(&path).unwrap()),
            Duration::from_millis(1),
        );
        let (seen, handled, _) = Seen::new("kept");
        feed.consume(Box::new(seen)).unwrap();
        feed.record(&card("c1"), Op::Upserted);
        assert!(feed.settled(SOON));
        assert_eq!(*handled.lock().unwrap(), [1]);
        feed.stop();
        assert!(feed.ended(SOON));
    }
    // A notice written while the consumer was gone.
    let store = Arc::new(Store::open(&path).unwrap());
    Feed::new(store, Duration::from_millis(1)).record(&card("c2"), Op::Upserted);
    let feed = Feed::new(
        Arc::new(Store::open(&path).unwrap()),
        Duration::from_millis(1),
    );
    let (seen, handled, _) = Seen::new("kept");
    feed.consume(Box::new(seen)).unwrap();
    feed.record(&card("c3"), Op::Upserted);
    assert!(feed.settled(SOON));
    assert_eq!(*handled.lock().unwrap(), [2, 3], "the missed one, then on");
    feed.stop();
}

#[test]
fn a_consumer_that_panics_on_a_notice_sets_it_aside_and_goes_on() {
    let feed = Feed::new(Arc::new(Store::memory().unwrap()), Duration::from_millis(1));
    let (mut brittle, handled, _) = Seen::new("brittle");
    brittle.fails = Box::new(|notice| {
        assert_ne!(notice.seq, 1, "a bug in the handler");
        false
    });
    feed.consume(Box::new(brittle)).unwrap();
    feed.record(&card("c1"), Op::Upserted);
    feed.record(&card("c2"), Op::Upserted);
    assert!(feed.settled(SOON));
    assert_eq!(
        *handled.lock().unwrap(),
        [2],
        "the thread outlived the panic"
    );
    assert_eq!(feed.health()[0].dead_letters, 1);
    feed.stop();
    assert!(feed.ended(SOON));
}

#[test]
fn a_write_that_steps_over_a_thousand_prunes() {
    let feed = Feed::new(Arc::new(Store::memory().unwrap()), Duration::from_millis(1));
    feed.write(|write| {
        (0..999).try_for_each(|n| write.record(&card(&format!("c{n}")), Op::Upserted))
    })
    .unwrap();
    // Every notice so far was read long ago.
    feed.store()
        .write(|tx| {
            tx.execute("UPDATE change_events SET at = 0", [])
                .map(drop)
                .map_err(|why| why.to_string())
        })
        .unwrap();
    // Seqs 1000 and 1001: the write ends on neither a multiple of a thousand nor on the one before.
    feed.write(|write| {
        write.record(&card("c1"), Op::Upserted)?;
        write.record(&card("c2"), Op::Upserted)
    })
    .unwrap();
    // The prune runs on a thread of its own: the writer does not wait for it.
    let old = || -> u64 {
        feed.store()
            .read(|db| {
                db.query_row(
                    "SELECT COUNT(*) FROM change_events WHERE at = 0",
                    [],
                    |row| row.get(0),
                )
                .map_err(|why| why.to_string())
            })
            .unwrap()
    };
    let deadline = Instant::now() + SOON;
    while old() > 0 && Instant::now() < deadline {
        crate::backoff(Duration::from_millis(5));
    }
    assert_eq!(old(), 0);
}
