//! The change feed: the desk's one way to say that something changed, and the one way every part
//! that cares learns of it.
//!
//! An owner records a thin notice, `{ entity, op }`: what changed and how, never what it now
//! holds. An owner that keeps its state in the store (a board) records it in the same transaction
//! as the change ([`Feed::write`]), so a notice is there exactly when its change is: the feed is an
//! outbox. An owner that keeps its state in memory (the tabs, the terminals, the person's notes on
//! their way) records it under the lock of its change ([`Feed::record`]). The notices and each
//! consumer's cursor are kept in the store, so after a restart every consumer goes on from where it
//! was. A consumer reads what it needs from the owner when it handles the notice, so a
//! notice delivered twice, or after a newer one, does no harm. Delivery is at least once.
//!
//! Every consumer has a name, a kind and a cursor of its own, on a thread of its own: none waits
//! for another. A handler that fails is tried again on the same notice; after a few attempts the
//! notice is set aside (a dead letter), and the consumer reconciles from the owners instead. A
//! consumer that fell behind what the feed keeps reconciles the same way.
//!
//! The kinds are one enum, [`Entity`], and every consumer matches it whole (the crate denies a
//! `_` arm on its own enums): a kind added fails to build until each consumer says what it does
//! with it, or that it ignores it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;
#[cfg(test)]
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::now;
use crate::store::{Store, Transaction, params};

/// What a notice is about: each kind of thing the desk's owners announce, with its ref (ids and
/// owner, never content).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entity {
    /// A note the person sent from the window to an owner's agent (the registry keeps it until
    /// it is delivered).
    Said { owner: String, id: u64 },
    /// A pane of the window (`w1`, `t2`, `b1`) in this run, and whose it was.
    Pane { owner: Option<String>, key: String },
    /// A window: what it shows, where it is (docs/windows.md).
    Window { id: crate::layout::WindowId },
    /// A window's workspace of a project: its tabs, their places, the one shown.
    Workspace { id: crate::layout::WorkspaceId },
    /// A tab: its tree of panes, the one in focus.
    Tab { id: crate::layout::TabId },
    /// A project's board: one of its cards changed.
    Board { project: crate::projects::ProjectId },
    /// A terminal's program.
    Terminal { id: u32 },
    /// What an owner's agent asked the person.
    Ask { owner: String },
}

impl Entity {
    /// Whether what it names outlives the app. A board and the layout (windows, workspaces, tabs) do. A pane's key, a terminal, a
    /// note on its way and an ask do not, and their ids are given again after a
    /// restart: a notice of one from an earlier run names something else, and is passed over.
    pub fn outlives_the_run(&self) -> bool {
        match self {
            Entity::Board { .. }
            | Entity::Window { .. }
            | Entity::Workspace { .. }
            | Entity::Tab { .. } => true,
            Entity::Said { .. }
            | Entity::Pane { .. }
            | Entity::Terminal { .. }
            | Entity::Ask { .. } => false,
        }
    }
}

/// How it changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// Created or changed: read it again.
    Upserted,
    /// Gone: the entity's ref is all there is to go by.
    Deleted,
}

/// One notice, numbered in the order it was recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub seq: u64,
    pub entity: Entity,
    pub op: Op,
}

/// What a consumer is for: a projector keeps derived data that can be rebuilt from the owners; a
/// reactor does something about a change, and is safe to repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Projector,
    Reactor,
}

/// A reader of the feed.
pub trait Consumer: Send {
    /// Its name: one cursor, one thread, one health line.
    fn name(&self) -> &'static str;
    fn kind(&self) -> Kind;
    /// Handle one notice. Running it twice, or after a newer notice, must leave the same result.
    ///
    /// # Errors
    /// Why it could not: the notice is tried again, then set aside.
    fn handle(&mut self, notice: &Notice) -> Result<(), String>;
    /// Re-derive from the owners what notices would have said: when the consumer starts, after a
    /// notice set aside, and after it fell behind what the feed keeps.
    fn reconcile(&mut self) {}
}

/// One consumer's state, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Health {
    pub name: &'static str,
    pub kind: Kind,
    /// The next notice it reads.
    pub cursor: u64,
    /// Failed attempts on the notice at its cursor.
    pub attempts: u32,
    pub last_error: Option<String>,
    /// Notices it set aside.
    pub dead_letters: u64,
}

/// Notices kept for a consumer that is behind; past this, the oldest go and it reconciles.
const KEPT: u64 = 10_000;

/// How long a notice every consumer has read is kept, in seconds: a day.
const READ_KEPT: u64 = 24 * 3600;

/// How long a cursor nobody moved is kept, in seconds: a month.
const CURSOR_KEPT: u64 = 30 * 24 * 3600;

/// The feed, shared by owners and consumers.
#[derive(Clone)]
pub struct Feed {
    inner: Arc<Inner>,
}

struct Inner {
    store: Arc<Store>,
    /// This run of the desk: one more than the last run the store saw.
    run: u64,
    log: Mutex<Log>,
    /// Woken when a notice is recorded, a cursor moves, or the feed stops.
    changed: Condvar,
    retry: Duration,
}

/// What the feed holds in memory: where it stands, and how each consumer is.
#[derive(Default)]
struct Log {
    /// The seq the next notice gets.
    next: u64,
    health: HashMap<&'static str, Health>,
    stopped: bool,
    /// Consumers whose thread still runs.
    running: usize,
}

/// What an owner writes in the transaction of its change: its own rows, and the notices of them.
pub struct Write<'a, 'tx> {
    tx: &'a Transaction<'tx>,
    run: u64,
    recorded: AtomicU64,
}

impl<'tx> Write<'_, 'tx> {
    /// The transaction, for the owner's own rows.
    pub fn tx(&self) -> &Transaction<'tx> {
        self.tx
    }

    /// That `entity` changed by `op`, written with the change.
    ///
    /// # Errors
    /// SQLite could not write it: the whole write is undone.
    pub fn record(&self, entity: &Entity, op: Op) -> Result<(), String> {
        let entity = serde_json::to_string(entity).map_err(|why| why.to_string())?;
        let op = serde_json::to_string(&op).map_err(|why| why.to_string())?;
        self.tx
            .execute(
                "INSERT INTO change_events (entity, op, at, run) VALUES (?1, ?2, ?3, ?4)",
                params![entity, op, now(), self.run],
            )
            .map_err(|why| why.to_string())?;
        let seq = u64::try_from(self.tx.last_insert_rowid()).unwrap_or(0);
        self.recorded.store(seq, Ordering::Relaxed);
        Ok(())
    }
}

impl Feed {
    /// The feed kept in `store`, whose consumers try a failed notice again after `retry`, doubled
    /// each time.
    pub fn new(store: Arc<Store>, retry: Duration) -> Self {
        let (last, run): (u64, u64) = store
            .read(|db| {
                db.query_row(
                    "SELECT (SELECT seq FROM sqlite_sequence WHERE name = 'change_events'),
                            (SELECT MAX(run) FROM change_events)",
                    [],
                    |row| Ok((row.get::<_, Option<u64>>(0)?, row.get::<_, Option<u64>>(1)?)),
                )
                .map(|(last, run)| (last.unwrap_or(0), run.unwrap_or(0)))
                .map_err(|why| why.to_string())
            })
            .unwrap_or((0, 0));
        let log = Log {
            next: last.saturating_add(1),
            ..Log::default()
        };
        Self {
            inner: Arc::new(Inner {
                store,
                run: run.saturating_add(1),
                log: Mutex::new(log),
                changed: Condvar::new(),
                retry,
            }),
        }
    }

    /// The store the feed is kept in, for the owners that keep their rows beside it.
    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    fn log(&self) -> MutexGuard<'_, Log> {
        crate::lock(&self.inner.log)
    }

    /// `write` done in one transaction with the notices it records: both or neither. The
    /// consumers are woken once it is committed.
    ///
    /// # Errors
    /// What `write` answered, or why SQLite could not: nothing of it is kept.
    pub fn write<T>(
        &self,
        write: impl FnOnce(&Write<'_, '_>) -> Result<T, String>,
    ) -> Result<T, String> {
        if self.log().stopped {
            return Err("the desk is stopping".to_string());
        }
        let (answer, last) = self.inner.store.write(|tx| {
            let writing = Write {
                tx,
                run: self.inner.run,
                recorded: AtomicU64::new(0),
            };
            let answer = write(&writing)?;
            Ok((answer, writing.recorded.load(Ordering::Relaxed)))
        })?;
        if last > 0 {
            let mut log = self.log();
            let before = log.next;
            log.next = before.max(last.saturating_add(1));
            // The seqs given so far are `next - 1` and below.
            let crossed = log.next.saturating_sub(1) / 1000 > before.saturating_sub(1) / 1000;
            drop(log);
            self.inner.changed.notify_all();
            // A write of several notices may step over a thousand: that it crossed one is what
            // counts, not that it ended on it.
            if crossed {
                self.prune();
            }
        }
        Ok(answer)
    }

    /// Record that `entity` changed by `op`, for an owner whose state is in memory: it calls this
    /// under the lock of the change, so notices are in the order of the changes. A notice that
    /// could not be written is logged; the consumers' reconcile closes the gap.
    pub fn record(&self, entity: &Entity, op: Op) {
        if self.log().stopped {
            return;
        }
        if let Err(why) = self.write(|write| write.record(entity, op)) {
            log::warn!("feed: a notice of {entity:?} was not recorded: {why}");
        }
    }

    /// Every consumer's state.
    #[cfg(test)]
    pub fn health(&self) -> Vec<Health> {
        let mut all: Vec<Health> = self.log().health.values().cloned().collect();
        all.sort_by_key(|health| health.name);
        all
    }

    /// Whether every consumer has read every notice recorded so far, waiting up to `most`.
    #[cfg(test)]
    pub fn settled(&self, most: Duration) -> bool {
        let deadline = Instant::now() + most;
        let mut log = self.log();
        loop {
            let next = log.next;
            if log.health.values().all(|health| health.cursor >= next) {
                return true;
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            log = crate::wait_timeout(&self.inner.changed, log, left);
        }
    }

    /// No more notices; every consumer ends once it has read what it was given.
    pub fn stop(&self) {
        self.log().stopped = true;
        self.inner.changed.notify_all();
    }

    /// Whether every consumer's thread has ended after [`Feed::stop`], waiting up to `most`.
    #[cfg(test)]
    pub fn ended(&self, most: Duration) -> bool {
        let deadline = Instant::now() + most;
        let mut log = self.log();
        while log.running > 0 {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            log = crate::wait_timeout(&self.inner.changed, log, left);
        }
        true
    }

    /// What the feed keeps no longer: the notices every running consumer read a day ago or
    /// more, past `KEPT` the oldest anyway, and the cursors nobody moved in a month.
    pub fn prune(&self) {
        let (slowest, next) = {
            let log = self.log();
            let slowest = log.health.values().map(|health| health.cursor).min();
            (slowest.unwrap_or(log.next), log.next)
        };
        let now = now();
        let pruned = self.inner.store.write(|tx| {
            let text = |why: rusqlite::Error| why.to_string();
            tx.execute(
                "DELETE FROM change_events WHERE seq < ?1 AND at <= ?2",
                params![slowest, now.saturating_sub(READ_KEPT)],
            )
            .map_err(text)?;
            tx.execute(
                "DELETE FROM change_events WHERE seq < ?1",
                params![next.saturating_sub(KEPT)],
            )
            .map_err(text)?;
            tx.execute(
                "DELETE FROM change_cursors WHERE moved <= ?1",
                params![now.saturating_sub(CURSOR_KEPT)],
            )
            .map_err(text)?;
            Ok(())
        });
        if let Err(why) = pruned {
            log::warn!("feed: it could not be pruned: {why}");
        }
    }
}

mod reading;

#[cfg(test)]
#[path = "feed_tests.rs"]
mod tests;
