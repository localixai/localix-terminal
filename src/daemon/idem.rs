//! A request with an idempotency key is done once: a retry (after a connection broke before the
//! answer arrived) gets the answer of the first, waiting for it when it is still running. The
//! cache is the daemon's, not a connection's, because the retry comes on a new connection; it is
//! bounded in entries and in bytes (screenshots).

use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::wire::{Blob, RpcError, code};

/// Answers kept.
const ENTRIES: usize = 128;
/// Bytes of pictures kept with them.
const BYTES: usize = 16 << 20;

/// What a request came to: its result or its error, and bytes that go after the result.
#[derive(Debug, Clone)]
pub(super) struct Outcome {
    pub result: Result<Value, RpcError>,
    pub blob: Option<Blob>,
}

impl Outcome {
    pub(super) fn failed(error: RpcError) -> Self {
        Self {
            result: Err(error),
            blob: None,
        }
    }
}

/// `work` run, a panic in it being the request's error: the daemon goes on.
pub(super) fn guarded(work: impl FnOnce() -> Outcome) -> Outcome {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|_| {
        Outcome::failed(RpcError::new(
            code::INTERNAL,
            "the daemon hit a bug answering this call; the daemon goes on",
        ))
    })
}

enum Slot {
    Running,
    Done(Arc<Outcome>),
}

#[derive(Default)]
struct Kept {
    slots: HashMap<String, Slot>,
    /// Finished keys, oldest first.
    order: VecDeque<String>,
    bytes: usize,
}

#[derive(Default)]
pub(super) struct Idem {
    kept: Mutex<Kept>,
    finished: Condvar,
}

impl Idem {
    /// The outcome of the request `key` names: `work` if it was never run, else what it came to.
    pub(super) fn run(&self, key: &str, work: impl FnOnce() -> Outcome) -> Arc<Outcome> {
        let mut kept = crate::lock(&self.kept);
        loop {
            match kept.slots.get(key) {
                Some(Slot::Done(done)) => return Arc::clone(done),
                Some(Slot::Running) => {
                    kept = crate::wait_timeout(&self.finished, kept, Duration::from_secs(1));
                }
                None => break,
            }
        }
        kept.slots.insert(key.to_string(), Slot::Running);
        drop(kept);
        let done = Arc::new(guarded(work));
        let mut kept = crate::lock(&self.kept);
        kept.bytes += done.blob.as_ref().map_or(0, |blob| blob.bytes.len());
        kept.slots
            .insert(key.to_string(), Slot::Done(Arc::clone(&done)));
        kept.order.push_back(key.to_string());
        while kept.order.len() > ENTRIES || (kept.bytes > BYTES && kept.order.len() > 1) {
            let Some(oldest) = kept.order.pop_front() else {
                break;
            };
            if let Some(Slot::Done(gone)) = kept.slots.remove(&oldest) {
                kept.bytes -= gone.blob.as_ref().map_or(0, |blob| blob.bytes.len());
            }
        }
        drop(kept);
        self.finished.notify_all();
        done
    }
}

#[cfg(test)]
#[path = "idem_tests.rs"]
mod tests;
