//! One connection: lines in, a message each, answered by id. `initialize` comes first;
//! the calls that take time run on threads of their own, so requests are pipelined and a ping is
//! answered while a long call runs; a writer thread (`Outbox`) puts answers and notifications on the
//! socket in whole messages.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::io::{self, BufReader, Read};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::hub::EVENTS;
use super::idem::{Outcome, guarded};
use super::{Shared, methods};
use crate::wire::pipe::Outbox;
use crate::wire::{Frame, Message, Next, PROTOCOL, RpcError, code, read_frame, respond};

/// Calls one connection may have running at once.
const IN_FLIGHT: usize = 32;

/// How long a connection may stay silent before it has said `initialize`: a client sends it at
/// once, and a peer that does not holds one of the few connections the app takes.
const HELLO: Duration = Duration::from_secs(10);

/// A connection's input until it has said `initialize`: every read waits only for what is left of
/// the time `HELLO` allows in all, so a peer that sends a byte or a junk line now and then does not
/// hold the connection longer than one that is silent.
struct Greeting {
    stream: UnixStream,
    /// When the time is up; none once the client has said hello.
    until: Option<Instant>,
}

impl Read for Greeting {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(until) = self.until {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            self.stream.set_read_timeout(Some(left))?;
        }
        self.stream.read(buf)
    }
}

/// The longest idempotency key taken; clients send 32 characters.
const KEY_MOST: usize = 128;

/// What the methods know of the connection they answer.
pub(super) struct Conn {
    pub(super) id: u64,
    pub(super) out: Outbox,
    /// The client announced it takes bytes after a response.
    pub(super) binary: AtomicBool,
    pub(super) ready: AtomicBool,
    pub(super) running: AtomicUsize,
    /// The process at the other end, when the system says: a call from a program of one of the
    /// window's terminals is known by it.
    pub(super) peer: Option<libc::pid_t>,
}

impl Conn {
    /// Whether a picture may go as bytes on this connection.
    pub(super) fn carries_bytes(&self) -> bool {
        self.binary.load(Ordering::SeqCst)
    }
}

/// Serve `stream` until it ends.
pub(super) fn serve(stream: UnixStream, shared: &Arc<Shared>) {
    let Ok(read) = stream.try_clone() else { return };
    let peer = crate::wire::peer::peer_pid(&stream);
    let Ok(out) = Outbox::spawn(stream, "desk-write") else {
        return;
    };
    let conn = Arc::new(Conn {
        id: shared.next_connection(),
        out,
        binary: AtomicBool::new(false),
        ready: AtomicBool::new(false),
        running: AtomicUsize::new(0),
        peer,
    });
    let mut input = BufReader::new(Greeting {
        stream: read,
        until: Some(Instant::now() + HELLO),
    });
    let mut waiting = true;
    loop {
        match read_frame(&mut input) {
            Ok(Next::Got(frame)) => {
                take(shared, &conn, frame);
                if waiting && conn.ready.load(Ordering::SeqCst) {
                    // Said hello: a client may now be silent as long as it likes (an MCP server).
                    waiting = false;
                    input.get_mut().until = None;
                    let _ = input.get_ref().stream.set_read_timeout(None);
                }
            }
            Ok(Next::Bad(error)) => {
                conn.out
                    .send(Message::failed(Value::Null, error).encode(None));
            }
            Ok(Next::End) | Err(_) => break,
        }
    }
    shared.hub.unsubscribe(conn.id);
}

/// A connection the app cannot take: told why in one line, not left to guess at a close.
pub(super) fn refuse(mut stream: &UnixStream, why: &str) {
    use std::io::Write as _;
    let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
    let error = RpcError::new(code::UNAVAILABLE, why);
    let _ = stream.write_all(&Message::failed(Value::Null, error).encode(None));
}

/// A message read: a request is answered, the rest is not the app's to answer.
fn take(shared: &Arc<Shared>, conn: &Arc<Conn>, frame: Frame) {
    let Message::Request { id, method, params } = frame.message else {
        return;
    };
    if method == "initialize" {
        let outcome = guarded(|| initialize(shared, conn, &params));
        answer(conn, &id, &outcome);
    } else if !conn.ready.load(Ordering::SeqCst) {
        let error = RpcError::new(code::NOT_INITIALIZED, "`initialize` comes first");
        answer(conn, &id, &Outcome::failed(error));
    } else if super::api::find(&method).is_some_and(|known| known.work) {
        spawn_work(shared, conn, id, method, params);
    } else {
        let outcome = run(shared, conn, &method, &params);
        answer(conn, &id, &outcome);
        if method == "app/quit" && outcome.result.is_ok() {
            // Only after the answer is out: the process may end the moment the app stops.
            conn.out.flush();
            shared.stop();
        }
    }
}

/// A call that takes time, on a thread of its own, within the connection's share of them.
fn spawn_work(shared: &Arc<Shared>, conn: &Arc<Conn>, id: Value, method: String, params: Value) {
    if conn.running.fetch_add(1, Ordering::SeqCst) >= IN_FLIGHT {
        conn.running.fetch_sub(1, Ordering::SeqCst);
        let busy = RpcError::new(
            code::UNAVAILABLE,
            format!("{IN_FLIGHT} calls are running here already"),
        );
        answer(conn, &id, &Outcome::failed(busy));
        return;
    }
    let (shared, conn_for_work) = (Arc::clone(shared), Arc::clone(conn));
    let reply_id = id.clone();
    let spawned = std::thread::Builder::new()
        .name("desk-call".into())
        .spawn(move || {
            let outcome = run(&shared, &conn_for_work, &method, &params);
            conn_for_work.running.fetch_sub(1, Ordering::SeqCst);
            answer(&conn_for_work, &id, &outcome);
        });
    if let Err(why) = spawned {
        conn.running.fetch_sub(1, Ordering::SeqCst);
        let error = RpcError::new(
            code::UNAVAILABLE,
            format!("no thread to run the call on ({why})"),
        );
        answer(conn, &reply_id, &Outcome::failed(error));
    }
}

/// A request done: a call that acts, once whatever its key says, when it has one (a read is just
/// done: its retry costs nothing, and keeping its answer would push out the calls that need
/// theirs kept); a panic is its error.
fn run(shared: &Shared, conn: &Conn, method: &str, params: &Value) -> Arc<Outcome> {
    let work = || methods::call(shared, conn, method, params);
    let Some(key) = params.get("idempotencyKey").and_then(Value::as_str) else {
        return Arc::new(guarded(work));
    };
    if key.len() > KEY_MOST {
        let why = format!("`idempotencyKey` is at most {KEY_MOST} bytes");
        return Arc::new(Outcome::failed(RpcError::new(code::BAD_PARAMS, why)));
    }
    // Methods the table does not know (a test build's probes) keep their key.
    if super::api::find(method).is_some_and(|known| !known.work) {
        return Arc::new(guarded(work));
    }
    shared.idem.run(&kept_key(method, params, key), work)
}

/// The key an answer is kept under: the client's, with what the retry repeats (the method, the
/// project and the agent), so that no other caller's equal key finds it. What the caller wrote
/// is hashed, so that the key kept stays short however long the project or the agent is.
fn kept_key(method: &str, params: &Value, key: &str) -> String {
    let mut caller = DefaultHasher::new();
    method.hash(&mut caller);
    for name in ["project", "agent"] {
        params.get(name).and_then(Value::as_str).hash(&mut caller);
    }
    format!("{:016x}\0{key}", caller.finish())
}

fn answer(conn: &Conn, id: &Value, outcome: &Outcome) {
    conn.out
        .send(respond(id, &outcome.result, outcome.blob.as_ref()));
}

/// The handshake: both sides say their protocol, build and capabilities.
fn initialize(shared: &Shared, conn: &Conn, params: &Value) -> Outcome {
    let wanted = params
        .get("protocolVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if wanted < u64::from(PROTOCOL) {
        let why = format!("this app speaks protocol {PROTOCOL}; the client asked for {wanted}");
        return Outcome::failed(RpcError::new(code::UNAVAILABLE, why));
    }
    let binary = params
        .pointer("/capabilities/binary")
        .and_then(Value::as_bool)
        == Some(true);
    conn.binary.store(binary, Ordering::SeqCst);
    conn.ready.store(true, Ordering::SeqCst);
    let result = json!({
        "protocolVersion": PROTOCOL,
        "serverInfo": {
            "name": "llxterm",
            "version": env!("CARGO_PKG_VERSION"),
            "build": shared.build,
            "pid": std::process::id(),
        },
        "capabilities": {
            "binary": true,
            "pipelining": true,
            "idempotency": true,
            "notifications": EVENTS,
        },
    });
    Outcome {
        result: Ok(result),
        blob: None,
    }
}

#[cfg(test)]
#[path = "conn_tests.rs"]
mod tests;
