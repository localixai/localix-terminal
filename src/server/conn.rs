//! One connection: lines in, a message each, answered by id. `initialize` comes first and once;
//! the calls that take time run on threads of their own, so requests are pipelined and a ping is
//! answered while a long call runs; a writer thread (`Outbox`) puts answers and notifications on the
//! socket in whole messages.

use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use serde_json::{Value, json};

use super::hub::EVENTS;
use super::idem::{Outcome, guarded};
use super::{Shared, methods};
use crate::wire::pipe::Outbox;
use crate::wire::{Frame, Message, Next, PROTOCOL, RpcError, code, read_frame};

/// Calls one connection may have running at once.
const IN_FLIGHT: usize = 32;

/// What the methods know of the connection they answer.
pub(super) struct Conn {
    pub(super) id: u64,
    pub(super) out: Outbox,
    /// The client announced it takes bytes after a response.
    pub(super) binary: AtomicBool,
    pub(super) ready: AtomicBool,
    pub(super) running: AtomicUsize,
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
    let Ok(out) = Outbox::spawn(stream, "desk-write") else {
        return;
    };
    let conn = Arc::new(Conn {
        id: shared.next_connection(),
        out,
        binary: AtomicBool::new(false),
        ready: AtomicBool::new(false),
        running: AtomicUsize::new(0),
    });
    let mut input = BufReader::new(read);
    loop {
        match read_frame(&mut input) {
            Ok(Next::Got(frame)) => take(shared, &conn, frame),
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
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(200)));
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
        answer(conn, id, &outcome);
    } else if !conn.ready.load(Ordering::SeqCst) {
        let error = RpcError::new(code::NOT_INITIALIZED, "`initialize` comes first");
        answer(conn, id, &Outcome::failed(error));
    } else if super::api::find(&method).is_some_and(|known| known.work) {
        spawn_work(shared, conn, id, method, params);
    } else {
        let outcome = run(shared, conn, &method, &params);
        answer(conn, id, &outcome);
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
        answer(conn, id, &Outcome::failed(busy));
        return;
    }
    let (shared, conn_for_work) = (Arc::clone(shared), Arc::clone(conn));
    let reply_id = id.clone();
    let spawned = std::thread::Builder::new()
        .name("desk-call".into())
        .spawn(move || {
            let outcome = run(&shared, &conn_for_work, &method, &params);
            conn_for_work.running.fetch_sub(1, Ordering::SeqCst);
            answer(&conn_for_work, id, &outcome);
        });
    if let Err(why) = spawned {
        conn.running.fetch_sub(1, Ordering::SeqCst);
        let error = RpcError::new(
            code::UNAVAILABLE,
            format!("no thread to run the call on ({why})"),
        );
        answer(conn, reply_id, &Outcome::failed(error));
    }
}

/// A request done: once, whatever its key says, when it has one; a panic is its error.
fn run(shared: &Shared, conn: &Conn, method: &str, params: &Value) -> Arc<Outcome> {
    let work = || methods::call(shared, conn, method, params);
    match params.get("idempotencyKey").and_then(Value::as_str) {
        Some(key) => shared.idem.run(key, work),
        None => Arc::new(guarded(work)),
    }
}

fn answer(conn: &Conn, id: Value, outcome: &Outcome) {
    let message = match &outcome.result {
        Ok(result) => Message::done(id, result.clone()),
        Err(error) => Message::failed(id, error.clone()),
    };
    conn.out.send(message.encode(outcome.blob.as_ref()));
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
            "name": "llx-desk",
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
