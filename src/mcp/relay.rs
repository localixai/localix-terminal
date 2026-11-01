//! The relay's one connection to the daemon, kept for the whole session. A thread of its own
//! connects (starting the daemon once a call needs it), subscribes, pings every few seconds,
//! and connects again with a backoff when the connection breaks or a ping goes unanswered. A call
//! waits for a connection up to a short limit and is sent again, with the same idempotency key,
//! when the connection broke under it; past the limit it fails with the reason, never hangs.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::client::{Answer, Client, Error, Notes, Start};
use crate::wire::RpcError;

/// Times that matter, as values: a test shortens them.
#[derive(Clone, Copy)]
pub(super) struct Timing {
    pub(super) ping_every: Duration,
    pub(super) ping_timeout: Duration,
    /// How long a call waits for a connection before it fails.
    pub(super) reconnect_wait: Duration,
    pub(super) backoff_first: Duration,
    pub(super) backoff_last: Duration,
}

impl Timing {
    pub(super) const fn standard() -> Self {
        Self {
            ping_every: Duration::from_secs(5),
            ping_timeout: Duration::from_secs(3),
            reconnect_wait: Duration::from_secs(15),
            backoff_first: Duration::from_millis(100),
            backoff_last: Duration::from_secs(2),
        }
    }
}

/// Why a call has no answer.
#[derive(Debug)]
pub(super) enum Failure {
    /// The daemon answered with an error.
    Refused(RpcError),
    /// No connection could be had in time, or it kept breaking.
    Unavailable(String),
    /// No daemon runs, and the call was not one that may start it.
    Absent,
}

/// Told the text of each step a call's goal takes.
pub(super) type Sink = Arc<dyn Fn(&str) + Send + Sync>;

/// How a connection is made: whether it may start the daemon.
pub(super) type Connect = dyn Fn(Start, &Notes) -> Result<Option<Client>, String> + Send + Sync;

/// A call that finds its connection broken is made again this many times.
const ATTEMPTS: usize = 4;

#[derive(Default)]
struct State {
    client: Option<Arc<Client>>,
    /// A call has needed the daemon: it may be started.
    wanted: bool,
    /// The last look for a daemon found none.
    absent: bool,
    closing: bool,
    /// Why the last attempt to connect failed.
    trouble: String,
}

struct Inner {
    owner: String,
    timing: Timing,
    connect: Box<Connect>,
    state: Mutex<State>,
    /// Rung when the connection comes or goes, or a call asks for the daemon.
    changed: Condvar,
    /// A call's goal steps go to the sink under its idempotency key.
    sinks: Mutex<BTreeMap<String, Sink>>,
    stopped: AtomicBool,
}

#[derive(Clone)]
pub(super) struct Relay(Arc<Inner>);

impl Relay {
    /// The relay for `owner`'s tabs, keeping its connection in a thread of its own.
    pub(super) fn start(
        owner: &str,
        timing: Timing,
        connect: Box<Connect>,
    ) -> std::io::Result<Self> {
        let inner = Arc::new(Inner {
            owner: owner.to_string(),
            timing,
            connect,
            state: Mutex::default(),
            changed: Condvar::new(),
            sinks: Mutex::default(),
            stopped: AtomicBool::new(false),
        });
        let keeper = Arc::clone(&inner);
        std::thread::Builder::new()
            .name("mcp-keeper".into())
            .spawn(move || keep(&keeper))?;
        Ok(Self(inner))
    }

    /// The relay ends: its connection is let go.
    pub(super) fn close(&self) {
        crate::lock(&self.0.state).closing = true;
        self.0.stopped.store(true, Ordering::SeqCst);
        self.0.changed.notify_all();
    }

    /// `method` asked of the daemon with `params`, which carry the call's idempotency key. With
    /// `Start::Never` a daemon that is not running is reported, not started.
    pub(super) fn call(
        &self,
        method: &str,
        params: &Value,
        sink: Option<Sink>,
        start: Start,
    ) -> Result<Answer, Failure> {
        let inner = &self.0;
        if start == Start::Spawn {
            let mut state = crate::lock(&inner.state);
            state.wanted = true;
            state.absent = false;
        }
        inner.changed.notify_all();
        let key = params["idempotencyKey"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if let Some(sink) = sink {
            crate::lock(&inner.sinks).insert(key.clone(), sink);
        }
        let answer = (0..ATTEMPTS).find_map(|_| match self.client(start) {
            Err(Failure::Absent) => Some(Err(Failure::Absent)),
            Err(other) => Some(Err(other)),
            Ok(client) => match client.request(method, params.clone()) {
                Ok(answer) => Some(Ok(answer)),
                Err(Error::Rpc(error)) => Some(Err(Failure::Refused(error))),
                Err(Error::Foreign(why)) => Some(Err(Failure::Unavailable(why))),
                // The connection broke under the call: it may have been done, and the same key
                // makes asking again safe.
                Err(Error::Broken(_) | Error::Timeout) => None,
            },
        });
        crate::lock(&inner.sinks).remove(&key);
        answer.unwrap_or_else(|| {
            Err(Failure::Unavailable(
                "the connection to the daemon kept breaking".into(),
            ))
        })
    }

    /// A connection that stands, waiting for one up to the limit.
    fn client(&self, start: Start) -> Result<Arc<Client>, Failure> {
        let inner = &self.0;
        let deadline = Instant::now() + inner.timing.reconnect_wait;
        let mut state = crate::lock(&inner.state);
        loop {
            if let Some(client) = state.client.as_ref().filter(|c| c.link.is_open()) {
                return Ok(Arc::clone(client));
            }
            if start == Start::Never && state.absent {
                return Err(Failure::Absent);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                let trouble = state.trouble.clone();
                return Err(Failure::Unavailable(format!(
                    "the daemon could not be reached within {}s{}",
                    inner.timing.reconnect_wait.as_secs().max(1),
                    if trouble.is_empty() {
                        String::new()
                    } else {
                        format!(": {trouble}")
                    }
                )));
            }
            state = crate::wait_timeout(&inner.changed, state, left);
        }
    }
}

/// The keeper: connect, hold, connect again, until the relay closes.
fn keep(inner: &Arc<Inner>) {
    let mut backoff = inner.timing.backoff_first;
    while !inner.stopped.load(Ordering::SeqCst) {
        let start = if crate::lock(&inner.state).wanted {
            Start::Spawn
        } else {
            Start::Never
        };
        let hearing = Arc::clone(inner);
        let notes: Notes = Arc::new(move |method, params| hear(&hearing, method, params));
        match (inner.connect)(start, &notes) {
            Ok(Some(client)) => {
                let client = Arc::new(client);
                if subscribe(inner, &client) {
                    backoff = inner.timing.backoff_first;
                    hold(inner, &client);
                }
            }
            // No daemon, and none is wanted yet: look again when a call asks for one.
            Ok(None) => {
                crate::lock(&inner.state).absent = true;
                inner.changed.notify_all();
            }
            Err(why) => crate::lock(&inner.state).trouble = why,
        }
        let mut state = crate::lock(&inner.state);
        state.client = None;
        if !state.closing {
            let idle = if state.wanted {
                backoff
            } else {
                inner.timing.backoff_last
            };
            drop(crate::wait_timeout(&inner.changed, state, idle));
        }
        backoff = (backoff * 2).min(inner.timing.backoff_last);
    }
}

/// Subscribe to what the daemon tells; whether it did.
fn subscribe(inner: &Arc<Inner>, client: &Arc<Client>) -> bool {
    let events = json!([
        "goal/step",
        "tab/closed",
        "window/closed",
        "daemon/stopping"
    ]);
    let subscribed = client.request(
        "events/subscribe",
        json!({"owner": inner.owner, "events": events}),
    );
    if let Err(why) = subscribed {
        crate::lock(&inner.state).trouble = why.to_string();
        return false;
    }
    let mut state = crate::lock(&inner.state);
    state.client = Some(Arc::clone(client));
    state.absent = false;
    state.trouble.clear();
    drop(state);
    inner.changed.notify_all();
    true
}

/// Ping while the connection stands; return when it has ended or a ping went unanswered.
fn hold(inner: &Inner, client: &Client) {
    while !inner.stopped.load(Ordering::SeqCst) {
        if client.link.wait_closed(inner.timing.ping_every) {
            break;
        }
        if client
            .request_within("ping", json!({}), inner.timing.ping_timeout)
            .is_err()
        {
            client.link.shutdown();
            break;
        }
    }
    client.link.shutdown();
    inner.changed.notify_all();
}

/// A notification: a goal's step goes to the sink of the call that runs it.
fn hear(inner: &Inner, method: &str, params: &Value) {
    if method != "goal/step" {
        return;
    }
    let key = params["key"].as_str().unwrap_or_default();
    let sink = crate::lock(&inner.sinks).get(key).cloned();
    if let Some(sink) = sink {
        let text = params["text"].as_str().unwrap_or_default();
        sink(&format!(
            "{}: {text}",
            params["tab"].as_str().unwrap_or("?")
        ));
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
