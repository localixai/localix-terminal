//! Who asked to be told what. A connection subscribes to events (all of an owner's, or every
//! owner's) and the hub writes each notification to the ones it concerns. Writing never waits:
//! a connection whose queue is full misses the notification, and a comment it missed is told
//! again when it subscribes (comments are kept until acknowledged).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde_json::Value;

use crate::desk::Events;
use crate::wire::Message;
use crate::wire::pipe::Outbox;

/// The notifications the daemon sends.
pub(super) const EVENTS: [&str; 5] = [
    "comment/added",
    "tab/closed",
    "goal/step",
    "window/closed",
    "daemon/stopping",
];

struct Subscription {
    out: Outbox,
    /// Only this owner's events (and the ones that have no owner); none: every owner's.
    owner: Option<String>,
    events: BTreeSet<String>,
}

#[derive(Default)]
pub(super) struct Hub {
    subscriptions: Mutex<BTreeMap<u64, Subscription>>,
}

impl Hub {
    /// Connection `conn` is told of `events`, replacing what it asked before.
    pub(super) fn subscribe(
        &self,
        conn: u64,
        out: Outbox,
        owner: Option<String>,
        events: &[String],
    ) {
        let events = events.iter().cloned().collect();
        let sub = Subscription { out, owner, events };
        crate::lock(&self.subscriptions).insert(conn, sub);
    }

    pub(super) fn unsubscribe(&self, conn: u64) {
        crate::lock(&self.subscriptions).remove(&conn);
    }

    /// Connections subscribed to something.
    pub(super) fn count(&self) -> usize {
        crate::lock(&self.subscriptions).len()
    }

    /// Wait (each a bounded while, all at once) until what was queued for every subscriber has
    /// been written: the last word before the daemon exits reaches them.
    pub(super) fn flush(&self) {
        let outs: Vec<Outbox> = crate::lock(&self.subscriptions)
            .values()
            .map(|sub| sub.out.clone())
            .collect();
        std::thread::scope(|scope| {
            for out in &outs {
                let started = std::thread::Builder::new()
                    .name("desk-flush".into())
                    .spawn_scoped(scope, || out.flush());
                if started.is_err() {
                    out.flush();
                }
            }
        });
    }

    /// `event` to connection `conn` alone, if it subscribed to it: what answers its own call.
    pub(super) fn tell(&self, conn: u64, event: &str, params: Value) {
        let out = {
            let subs = crate::lock(&self.subscriptions);
            subs.get(&conn)
                .filter(|sub| sub.events.contains(event))
                .map(|sub| sub.out.clone())
        };
        if let Some(out) = out {
            out.offer(Message::notification(event, params).encode(None));
        }
    }
}

impl Events for Hub {
    fn publish(&self, event: &str, owner: Option<&str>, params: Value) {
        // Who is concerned is copied out, so no lock is held while the queues are written.
        let outs: Vec<Outbox> = crate::lock(&self.subscriptions)
            .values()
            .filter(|sub| sub.events.contains(event))
            .filter(|sub| match (&sub.owner, owner) {
                (Some(mine), Some(theirs)) => mine == theirs,
                (None, _) | (Some(_), None) => true,
            })
            .map(|sub| sub.out.clone())
            .collect();
        let bytes = Message::notification(event, params).encode(None);
        for out in outs {
            // A queue that is full is a client that does not read: it misses this one.
            out.offer(bytes.clone());
        }
    }
}

#[cfg(test)]
#[path = "hub_tests.rs"]
mod tests;
