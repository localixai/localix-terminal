//! The desk's writer thread: what the window tells is written to the store there, in the order
//! it was told, so the window's loop never waits on the disk. What the window's events change in
//! memory changes at once, on the window's thread; only the writes wait their turn.

use std::sync::Arc;
use std::sync::mpsc::{SyncSender, TrySendError};
use std::time::{Duration, Instant};

use super::Desk;
use crate::feed::{Entity, Op};

/// How many writes wait for the writer thread at most.
const WRITES: usize = 256;

/// How long a write waits for room when the queue is full (the disk far behind), before it is
/// done in place.
const ROOM: Duration = Duration::from_secs(2);

/// What the writer thread is handed.
pub(super) type Writes = SyncSender<Box<dyn FnOnce() + Send>>;

impl Desk {
    /// `write` done on the desk's writer thread, after what was handed it before: the window's
    /// loop goes on at once. With no thread to be had, it is done here.
    pub(super) fn write_later(&self, write: impl FnOnce() + Send + 'static) {
        let writes = self.writes.get_or_init(start);
        let mut write: Box<dyn FnOnce() + Send> = Box::new(write);
        let deadline = Instant::now() + ROOM;
        // A queue full waits a little for room, so a write never goes ahead of older ones; only a
        // disk that stays far behind has it done here, rather than the window's loop stopping.
        loop {
            match writes.try_send(write) {
                Ok(()) => return,
                Err(TrySendError::Full(again)) if Instant::now() < deadline => {
                    write = again;
                    crate::backoff(Duration::from_millis(1));
                }
                Err(TrySendError::Full(write)) => {
                    log::warn!("desk: the writer is far behind: a write is done in place");
                    return write();
                }
                Err(TrySendError::Disconnected(write)) => return write(),
            }
        }
    }

    /// Whether every write handed to the writer thread before this call is done, waiting up to
    /// `most`.
    pub(super) fn writes_done(&self, most: Duration) -> bool {
        if self.writes.get().is_none() {
            return true;
        }
        let (done, finished) = std::sync::mpsc::sync_channel(1);
        self.write_later(move || {
            let _ = done.send(());
        });
        finished.recv_timeout(most).is_ok()
    }

    /// Pane `key` the person closed, `owner`'s: let go of in memory now, its row closed and its
    /// end told (to none for a board, which stays) on the writer thread.
    pub(super) fn closed_later(self: &Arc<Self>, key: &str, owner: Option<String>, told: bool) {
        let closing = self.closing(key);
        let (desk, key) = (Arc::clone(self), key.to_string());
        self.write_later(move || {
            if let Some(closing) = closing {
                desk.close_row(&key, &closing);
            }
            if told {
                desk.feed.record(&Entity::Pane { owner, key }, Op::Deleted);
            }
        });
    }
}

/// The writer thread, started with the first write; none to be had, each write is done in place.
fn start() -> Writes {
    let (send, take) = std::sync::mpsc::sync_channel::<Box<dyn FnOnce() + Send>>(WRITES);
    let started = std::thread::Builder::new()
        .name("desk-writes".into())
        .spawn(move || {
            for write in take {
                // A write that panics is a bug of its own: the ones after it still go.
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(write)).is_err() {
                    log::warn!("desk: a write panicked: the writer goes on");
                }
            }
        });
    if let Err(why) = started {
        log::warn!("desk: no writer thread ({why}): writes are done in place");
    }
    send
}

#[cfg(test)]
#[path = "writes_tests.rs"]
mod tests;
