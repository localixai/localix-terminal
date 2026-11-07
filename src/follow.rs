//! `window --said --follow`: what the person sends from the window, printed as it comes, so an
//! agent that watches a command's output wakes on each comment. A
//! subscription to the app's `comment/added`, not a poll; a note printed is acknowledged, so
//! the app does not give it again.

use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use serde_json::{Value, json};

use crate::client::{Caller, Client, Notes, Start};

/// How long to wait for a note before looking at whether the connection stands.
const LOOK: Duration = Duration::from_secs(2);
/// Between attempts to reach an app that is not there.
const RETRY: Duration = Duration::from_millis(500);

/// Runs until the monitor that started it stops it, or is gone (an agent's session that ended
/// without stopping it: it would wait for the app forever, printing to no one); then answers
/// the exit code, 0. An app not running yet, or gone, is waited for, so the follower can
/// start before the first tab and outlive an app restart.
/// With `lines`, each note is one JSON string on a line of its own, for a program that reads
/// it: a note's lines never come apart.
pub fn follow(caller: &Caller, lines: bool) -> u8 {
    let parent = parent();
    loop {
        // Unbounded, so a note is never dropped: the callback runs on the connection's reader
        // thread, which must not wait on a full queue (it reads the replies too). It cannot grow
        // past what the app holds pending, a few small notes.
        #[expect(
            clippy::disallowed_methods,
            reason = "a send that never waits, on the reader thread; bounded by the notes pending in the app"
        )]
        let (notes, arrived) = channel::<Value>();
        let hear: Notes = Arc::new(move |method, params| {
            if method == "comment/added" {
                let _ = notes.send(params.clone());
            }
        });
        // Never starts the app: a follower waits for one.
        if let Ok(Some(client)) = Client::connect(Start::Never, &hear) {
            let events = json!(["comment/added", "app/quitting"]);
            let subscribed =
                client.request("events/subscribe", caller.with(json!({"events": events})));
            while subscribed.is_ok() {
                match arrived.recv_timeout(LOOK) {
                    Ok(note) => print(&client, caller, &note, lines),
                    Err(RecvTimeoutError::Timeout) if client.link.is_open() => {
                        if left(parent) {
                            return 0;
                        }
                    }
                    Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
                }
            }
        }
        if left(parent) {
            return 0;
        }
        pause();
    }
}

/// The process that started this one.
fn parent() -> libc::pid_t {
    // SAFETY: getppid only reads this process's parent; it cannot fail.
    unsafe { libc::getppid() }
}

/// Whether `parent`, the monitor, is gone: an orphan is given to another parent (the system's
/// first process, when it was gone already as this one began).
fn left(parent: libc::pid_t) -> bool {
    let now = self::parent();
    now != parent || now == 1
}

/// One note, as one block written at once (the monitor reads lines; stdout's line buffer would
/// let out each line on its own), then acknowledged. With `lines`, the block as a JSON string.
fn print(client: &Client, caller: &Caller, note: &Value, lines: bool) {
    let text = note["text"].as_str().unwrap_or_default().to_string();
    let block = crate::desk::said_block(&[text]);
    let shown = if lines {
        serde_json::to_string(&block).unwrap_or_default()
    } else {
        block
    };
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(format!("{shown}\n").as_bytes());
    let _ = out.flush();
    drop(out);
    let _ = client.request("comment/ack", caller.with(json!({"ids": [note["id"]]})));
}

#[expect(
    clippy::disallowed_methods,
    reason = "a backoff before the app is looked for again: there is no event to wait on"
)]
fn pause() {
    std::thread::sleep(RETRY);
}
