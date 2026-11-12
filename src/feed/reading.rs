//! The consumers' side of the feed: each started on a thread of its own, reading pages past its
//! cursor, handling each notice with retries, and moving its cursor in the store.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use super::{Consumer, Entity, Feed, Health, Notice, Op, now};
use crate::store::{OptionalExtension, params};

/// Attempts on one notice before it is set aside.
const ATTEMPTS: u32 = 5;

/// Notices a consumer reads at once.
const PAGE: u32 = 100;

/// How long a consumer waits before it asks the store again, when the store gave nothing though
/// notices are there (a read that failed).
const READ_RETRY: Duration = Duration::from_secs(1);

/// `work` run, a panic in it being an error: a consumer's thread goes on.
fn guarded<T>(work: impl FnOnce() -> T) -> Result<T, String> {
    match catch_unwind(AssertUnwindSafe(work)) {
        Ok(done) => Ok(done),
        Err(_) => Err("panicked".to_string()),
    }
}

/// `consumer` reconciled; a panic in it is logged, and the consumer goes on.
fn reconcile(consumer: &mut dyn Consumer) {
    if let Err(why) = guarded(|| consumer.reconcile()) {
        log::warn!("feed consumer {} {why} reconciling", consumer.name());
    }
}

/// A consumer's thread, counted while it runs: counted out however the thread ends.
struct Running(Feed);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.log().running -= 1;
        self.0.inner.changed.notify_all();
    }
}

/// A row of the feed as a consumer's page has it.
enum Item {
    Notice(Notice),
    /// A notice of something that did not outlive an earlier run.
    Gone,
    /// A row this build does not read.
    Unread,
}

impl Feed {
    /// Start `consumer` on a thread of its own, after it reconciles: from its cursor, kept from
    /// before, or from now on when it is new.
    ///
    /// # Errors
    /// The system gave no thread.
    pub fn consume(&self, mut consumer: Box<dyn Consumer>) -> std::io::Result<()> {
        let name = consumer.name();
        let kept: Option<u64> = (self.inner.store)
            .read(|db| {
                db.query_row(
                    "SELECT seq FROM change_cursors WHERE name = ?1",
                    [name],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|why| why.to_string())
            })
            .ok()
            .flatten();
        {
            let mut log = self.log();
            let cursor = kept.unwrap_or(log.next).min(log.next);
            log.health.insert(
                name,
                Health {
                    name,
                    kind: consumer.kind(),
                    cursor,
                    attempts: 0,
                    last_error: None,
                    dead_letters: 0,
                },
            );
            log.running += 1;
        }
        let feed = self.clone();
        // Dropped with the closure when no thread is given, as when the thread ends.
        let running = Running(self.clone());
        std::thread::Builder::new()
            .name(format!("desk-feed-{name}"))
            .spawn(move || {
                let _running = running;
                reconcile(consumer.as_mut());
                feed.run(consumer.as_mut());
            })
            .map(drop)
    }

    /// The consumer's loop: a page past its cursor, each notice handled, the consumer told it
    /// has caught up, then the cursor kept, once a page: after a crash the page's notices come again, which a consumer takes as
    /// it takes any notice told twice.
    fn run(&self, consumer: &mut dyn Consumer) {
        let name = consumer.name();
        while let Some(page) = self.page(name, consumer) {
            let last = page.last().map(|(seq, _)| *seq);
            for (seq, item) in page {
                match item {
                    Item::Notice(notice) => self.deliver(name, consumer, &notice),
                    // Of a tab or a terminal of an earlier run: what it names is gone.
                    Item::Gone => {}
                    // A row this build does not read (a kind it does not know): passed over.
                    Item::Unread => {
                        log::warn!(
                            "feed consumer {name} passed over notice {seq}: it does not read"
                        );
                    }
                }
                if Some(seq) == last {
                    consumer.caught_up();
                }
                self.advanced(name, seq);
            }
            if let Some(seq) = last {
                self.keep_cursor(name, seq);
            }
        }
    }

    /// The notices past `name`'s cursor, waiting for one; `None` once the feed stopped. A cursor
    /// older than what the feed keeps jumps to the oldest kept, the consumer reconciling the gap.
    fn page(&self, name: &'static str, consumer: &mut dyn Consumer) -> Option<Vec<(u64, Item)>> {
        loop {
            let cursor = {
                let log = self.log();
                let cursor = (log.health.get(name)).map_or(log.next, |health| health.cursor);
                // Nothing past the cursor: waited for without asking the store.
                if log.next <= cursor {
                    if log.stopped {
                        return None;
                    }
                    drop(match self.inner.changed.wait(log) {
                        Ok(log) => log,
                        Err(poisoned) => poisoned.into_inner(),
                    });
                    continue;
                }
                cursor
            };
            let oldest = self.oldest().unwrap_or(cursor);
            if cursor < oldest {
                self.moved(name, oldest.saturating_sub(1));
                log::warn!("feed consumer {name} fell behind what the feed keeps: it reconciles");
                reconcile(consumer);
                continue;
            }
            let page = self.read_page(cursor);
            if !page.is_empty() {
                return Some(page);
            }
            let log = self.log();
            if log.stopped {
                return None;
            }
            // Notices are there but the store gave none (a read failed): a pause, not a spin.
            if log.next > cursor {
                drop(crate::wait_timeout(&self.inner.changed, log, READ_RETRY));
                continue;
            }
            drop(match self.inner.changed.wait(log) {
                Ok(log) => log,
                Err(poisoned) => poisoned.into_inner(),
            });
        }
    }

    /// The seq of the oldest notice kept.
    fn oldest(&self) -> Option<u64> {
        (self.inner.store)
            .read(|db| {
                db.query_row("SELECT MIN(seq) FROM change_events", [], |row| row.get(0))
                    .map_err(|why| why.to_string())
            })
            .ok()
            .flatten()
    }

    /// Up to [`PAGE`] notices from `cursor` on, as each reads.
    fn read_page(&self, cursor: u64) -> Vec<(u64, Item)> {
        let run = self.inner.run;
        let read = self.inner.store.read(|db| {
            let mut rows = db
                .prepare_cached(
                    "SELECT seq, entity, op, run FROM change_events
                     WHERE seq >= ?1 ORDER BY seq LIMIT ?2",
                )
                .map_err(|why| why.to_string())?;
            let page = rows
                .query_map(params![cursor, PAGE], |row| {
                    Ok((
                        row.get::<_, u64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, u64>(3)?,
                    ))
                })
                .map_err(|why| why.to_string())?
                .filter_map(Result::ok)
                .map(|(seq, entity, op, written)| {
                    let read = serde_json::from_str::<Entity>(&entity)
                        .ok()
                        .zip(serde_json::from_str::<Op>(&op).ok());
                    let item = match read {
                        None => Item::Unread,
                        Some((entity, _)) if written != run && !entity.outlives_the_run() => {
                            Item::Gone
                        }
                        Some((entity, op)) => Item::Notice(Notice { seq, entity, op }),
                    };
                    (seq, item)
                })
                .collect();
            Ok(page)
        });
        read.unwrap_or_else(|why| {
            log::warn!("feed: notices could not be read: {why}");
            vec![]
        })
    }

    /// `notice` handled, tried again on failure with a doubling pause, set aside after
    /// [`ATTEMPTS`]; then the cursor moves past it.
    fn deliver(&self, name: &'static str, consumer: &mut dyn Consumer, notice: &Notice) {
        let mut pause = self.inner.retry;
        loop {
            match guarded(|| consumer.handle(notice)).and_then(|handled| handled) {
                Ok(()) => break,
                Err(why) => {
                    let attempts = self.failed(name, &why);
                    if attempts >= ATTEMPTS {
                        log::warn!(
                            "feed consumer {name} set notice {} aside: {why}",
                            notice.seq
                        );
                        self.set_aside(name);
                        reconcile(consumer);
                        break;
                    }
                    if self.wait(pause) {
                        break;
                    }
                    pause = pause.saturating_mul(2);
                }
            }
        }
    }

    /// `name` has read the notice `seq`: its cursor moves past it, here and in the store.
    fn moved(&self, name: &'static str, seq: u64) {
        self.keep_cursor(name, seq);
        self.advanced(name, seq);
    }

    /// `name`'s cursor past `seq`, in the store.
    fn keep_cursor(&self, name: &'static str, seq: u64) {
        let cursor = seq.saturating_add(1);
        let kept = self.inner.store.write(|tx| {
            tx.execute(
                "INSERT INTO change_cursors (name, seq, moved) VALUES (?1, ?2, ?3)
                 ON CONFLICT (name) DO UPDATE SET seq = excluded.seq, moved = excluded.moved",
                params![name, cursor, now()],
            )
            .map(drop)
            .map_err(|why| why.to_string())
        });
        if let Err(why) = kept {
            log::warn!("feed consumer {name}: its cursor was not kept: {why}");
        }
    }

    /// `name`'s cursor past `seq`, as those who wait for it see it.
    fn advanced(&self, name: &'static str, seq: u64) {
        let cursor = seq.saturating_add(1);
        let mut log = self.log();
        if let Some(health) = log.health.get_mut(name) {
            health.cursor = cursor;
            health.attempts = 0;
        }
        drop(log);
        self.inner.changed.notify_all();
    }

    fn failed(&self, name: &'static str, why: &str) -> u32 {
        let mut log = self.log();
        let Some(health) = log.health.get_mut(name) else {
            return ATTEMPTS;
        };
        health.attempts = health.attempts.saturating_add(1);
        health.last_error = Some(why.to_string());
        health.attempts
    }

    fn set_aside(&self, name: &'static str) {
        if let Some(health) = self.log().health.get_mut(name) {
            health.dead_letters = health.dead_letters.saturating_add(1);
        }
    }

    /// A backoff before the next attempt, cut short when the feed stops: whether it stopped.
    fn wait(&self, pause: Duration) -> bool {
        let deadline = Instant::now() + pause;
        let mut log = self.log();
        while !log.stopped {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            log = crate::wait_timeout(&self.inner.changed, log, left);
        }
        true
    }
}
