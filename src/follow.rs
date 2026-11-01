//! `window --said --follow`: what the person sends from the window, printed as it comes, so a
//! monitor in the calling agent (Claude Code's `Monitor`) wakes it on each comment. A
//! subscription to the daemon's `comment/added`, not a poll; a note printed is acknowledged, so
//! the daemon does not give it again.

use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, sync_channel};
use std::time::Duration;

use serde_json::{Value, json};

use crate::client::{Client, Notes, Start};

/// How long to wait for a note before looking at whether the connection stands.
const LOOK: Duration = Duration::from_secs(2);
/// Between attempts to reach a daemon that is not there.
const RETRY: Duration = Duration::from_millis(500);

/// Never returns: the monitor that started it stops it. A daemon not running yet, or gone, is
/// waited for, so the follower can start before the first tab and outlive a daemon restart.
pub fn follow(owner: &str) -> ! {
    loop {
        let (notes, arrived) = sync_channel::<Value>(64);
        let hear: Notes = Arc::new(move |method, params| {
            if method == "comment/added" {
                let _ = notes.try_send(params.clone());
            }
        });
        // Never starts a daemon: a follower waits for one.
        if let Ok(Some(client)) = Client::connect(Start::Never, &hear) {
            let events = json!(["comment/added", "daemon/stopping"]);
            let subscribed = client.request(
                "events/subscribe",
                json!({"owner": owner, "events": events}),
            );
            while subscribed.is_ok() {
                match arrived.recv_timeout(LOOK) {
                    Ok(note) => print(&client, owner, &note),
                    Err(RecvTimeoutError::Timeout) if client.link.is_open() => {}
                    Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
                }
            }
        }
        pause();
    }
}

/// One note, as one block, flushed at once (the monitor reads lines), then acknowledged.
fn print(client: &Client, owner: &str, note: &Value) {
    let text = note["text"].as_str().unwrap_or_default().to_string();
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", crate::desk::said_block(&[text]));
    let _ = out.flush();
    drop(out);
    let _ = client.request("comment/ack", json!({"owner": owner, "ids": [note["id"]]}));
}

#[expect(
    clippy::disallowed_methods,
    reason = "a backoff before the daemon is looked for again: there is no event to wait on"
)]
fn pause() {
    std::thread::sleep(RETRY);
}
