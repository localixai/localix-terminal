//! A connection to the app, as the client holds it: requests go out with ids, a thread reads
//! what comes back and gives each answer to the call waiting for it, and the app's
//! notifications to whoever listens. Calls on one link may be made from many threads at once.

use std::collections::BTreeMap;
use std::io::BufReader;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::wire::pipe::Outbox;
use crate::wire::{Blob, Message, Next, RpcError, read_frame};

/// What a request came to.
#[derive(Debug, Clone)]
pub struct Answer {
    pub value: Value,
    pub blob: Option<Blob>,
}

/// Why a request has no answer.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The app answered with an error.
    Rpc(RpcError),
    /// The connection is gone, or was never right; the request may or may not have been done:
    /// ask again with the same idempotency key.
    Broken(String),
    /// No answer within the time given; the connection is still there.
    Timeout,
    /// The other end sent what is no protocol-2 message: an app of an older protocol.
    Foreign(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rpc(error) => f.write_str(&error.message),
            Self::Broken(why) | Self::Foreign(why) => f.write_str(why),
            Self::Timeout => f.write_str("the app did not answer in time"),
        }
    }
}

/// Told of each notification: its method and params.
pub type Notes = Arc<dyn Fn(&str, &Value) + Send + Sync>;

type Waiting = BTreeMap<u64, SyncSender<Result<Answer, Error>>>;

/// How a connection ended.
#[derive(Clone)]
enum Ended {
    Gone(String),
    Foreign(String),
}

impl Ended {
    fn error(&self) -> Error {
        match self {
            Self::Gone(why) => Error::Broken(why.clone()),
            Self::Foreign(why) => Error::Foreign(why.clone()),
        }
    }
}

#[derive(Default)]
struct Pending {
    waiting: Waiting,
    /// Why the connection ended, once it has.
    closed: Option<Ended>,
}

/// What the link and its reader share.
#[derive(Default)]
struct Shared {
    pending: Mutex<Pending>,
    /// Rung when the connection ends.
    ended: Condvar,
}

pub struct Link {
    stream: UnixStream,
    out: Outbox,
    shared: Arc<Shared>,
    next: AtomicU64,
}

impl Link {
    /// Start reading `stream`; `notes` hears the app's notifications on the reader's thread.
    pub fn open(stream: UnixStream, notes: Notes) -> std::io::Result<Self> {
        let read = stream.try_clone()?;
        let out = Outbox::spawn(stream.try_clone()?, "desk-send")?;
        let shared = Arc::new(Shared::default());
        let theirs = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("desk-recv".into())
            .spawn(move || {
                let ended = read_all(BufReader::new(read), &theirs, &notes);
                end(&theirs, &ended);
            })?;
        Ok(Self {
            stream,
            out,
            shared,
            next: AtomicU64::new(1),
        })
    }

    /// Whether the connection still stands.
    pub fn is_open(&self) -> bool {
        crate::lock(&self.shared.pending).closed.is_none()
    }

    /// Wait up to `limit` for the connection to end; whether it has.
    pub fn wait_closed(&self, limit: Duration) -> bool {
        let pending = crate::lock(&self.shared.pending);
        let pending = if pending.closed.is_some() {
            pending
        } else {
            crate::wait_timeout(&self.shared.ended, pending, limit)
        };
        pending.closed.is_some()
    }

    /// End the connection now: calls waiting for an answer are told it broke.
    pub fn shutdown(&self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }

    /// `method` asked, waiting `limit` for the answer (for ever without one).
    pub fn request(
        &self,
        method: &str,
        params: Value,
        limit: Option<Duration>,
    ) -> Result<Answer, Error> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (reply, answer) = sync_channel(1);
        {
            let mut pending = crate::lock(&self.shared.pending);
            if let Some(ended) = &pending.closed {
                return Err(ended.error());
            }
            pending.waiting.insert(id, reply);
        }
        if !self
            .out
            .send(Message::request(id, method, params).encode(None))
        {
            crate::lock(&self.shared.pending).waiting.remove(&id);
            return Err(Error::Broken("the app went away".into()));
        }
        let got = match limit {
            Some(limit) => answer.recv_timeout(limit).map_err(|_gone| Error::Timeout),
            None => answer
                .recv()
                .map_err(|_gone| Error::Broken("the app went away".into())),
        };
        if got.is_err() {
            crate::lock(&self.shared.pending).waiting.remove(&id);
        }
        got?
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// Everything the app says, until it stops saying it; why it stopped.
fn read_all(mut input: BufReader<UnixStream>, shared: &Shared, notes: &Notes) -> Ended {
    loop {
        match read_frame(&mut input) {
            Ok(Next::Got(frame)) => match frame.message {
                Message::Response { id, outcome } => {
                    let waiter = id
                        .as_u64()
                        .and_then(|id| crate::lock(&shared.pending).waiting.remove(&id));
                    let answer = outcome
                        .map(|value| Answer {
                            value,
                            blob: frame.blob,
                        })
                        .map_err(Error::Rpc);
                    if let Some(waiter) = waiter {
                        let _ = waiter.send(answer);
                    }
                }
                Message::Notification { method, params } => notes(&method, &params),
                Message::Request { .. } => {}
            },
            Ok(Next::Bad(error)) => {
                return Ended::Foreign(format!(
                    "the app speaks another protocol (an older build?): {}",
                    error.message
                ));
            }
            Ok(Next::End) => return Ended::Gone("the app went away".into()),
            Err(error) => return Ended::Gone(format!("the app went away ({error})")),
        }
    }
}

/// The connection is over: every call waiting is told.
fn end(shared: &Shared, ended: &Ended) {
    let mut pending = crate::lock(&shared.pending);
    pending.closed = Some(ended.clone());
    for waiter in std::mem::take(&mut pending.waiting).into_values() {
        let _ = waiter.send(Err(ended.error()));
    }
    drop(pending);
    shared.ended.notify_all();
}
