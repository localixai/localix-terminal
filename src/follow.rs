//! The followers: a subscription to the app's events that prints what an agent's program needs
//! as it comes, so it wakes on a change and never asks now and then. `window --said --follow`
//! prints what the person sends from the window; `window --watch` (watch.rs) prints the app's
//! state, the caller's tabs and board, and the person's comments, each a JSON line, for the
//! Claude Code plugin's line above the prompt. A note printed is acknowledged, so the app does
//! not give it again.
//!
//! A follower waits for an app that is not running yet, or gone, so it can start before the
//! first tab and outlive an app restart; and it ends with the program that started it (an
//! agent's session that ended without stopping it would wait for the app forever, printing to
//! no one). Both are waited on as events: the app's folder changing as its socket appears (the
//! files' watch), the parent's end (wake.rs). Where the system cannot say either, it looks now
//! and then.

mod wake;
mod watch;

use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use serde_json::{Value, json};

use crate::client::{Caller, Client, Notes, Start};

pub use watch::watch;

/// How often to look at whether the parent is gone, where its end cannot be waited on.
const LOOK: Duration = Duration::from_secs(2);
/// Between attempts to reach an app that is not there, where its socket cannot be waited on.
const RETRY: Duration = Duration::from_millis(500);
/// A socket that appeared answers within this many short waits, as the app starts listening.
const SOON: usize = 10;

/// What the follower's one loop wakes on.
enum Heard {
    /// A notification of connection `.0`: its method and params.
    Note(u64, String, Value),
    /// Connection `.0` ended.
    Closed(u64),
    /// The app's folder changed: its socket may be there.
    Appeared,
    /// The program that started this one ended.
    Orphaned,
}

/// What a follower is told, one at a time on its own thread, where it may call the app.
pub(crate) enum Step<'a> {
    /// The app answered and the subscription is made.
    Connected(&'a Client),
    /// A notification the follower subscribed to.
    Note(&'a Client, &'a str, &'a Value),
    /// The app is not there (yet, or any more).
    Away,
}

/// Told to the loop as the connection's reader lets go of it: the connection ended.
struct Closing(Sender<Heard>, u64);

impl Drop for Closing {
    fn drop(&mut self) {
        let _ = self.0.send(Heard::Closed(self.1));
    }
}

/// `window --said --follow`: each comment printed as one block, or with `lines` as one JSON
/// string on a line of its own (a note's lines never come apart for a program that reads it).
pub fn follow(caller: &Caller, lines: bool) -> u8 {
    run(caller, &["comment/added", "app/quitting"], |step| {
        if let Step::Note(client, "comment/added", note) = step {
            let block = said(client, caller, note);
            let shown = match lines {
                true => serde_json::to_string(&block).unwrap_or_default(),
                false => block,
            };
            printed(&shown);
        }
    })
}

/// The comment `note` as the agent reads it, acknowledged: the app does not give it again.
fn said(client: &Client, caller: &Caller, note: &Value) -> String {
    let text = note["text"].as_str().unwrap_or_default().to_string();
    let _ = client.request("comment/ack", caller.with(json!({"ids": [note["id"]]})));
    crate::desk::said_block(&[text])
}

/// `text` and a newline as one block written at once: the reader reads lines, and stdout's line
/// buffer would let out each line on its own.
fn printed(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(format!("{text}\n").as_bytes());
    let _ = out.flush();
}

/// The follower's loop: subscribed to `events` for `caller` whenever the app runs, each step
/// told to `on`; it ends, with 0, when the program that started it does.
fn run(caller: &Caller, events: &[&str], mut on: impl FnMut(Step<'_>)) -> u8 {
    // Unbounded, so a note is never dropped: the connection's reader thread sends into it and
    // must not wait on a full queue (it reads the replies too). It cannot grow past what the app
    // holds pending, a few small notes, and the folder's few changes.
    #[expect(
        clippy::disallowed_methods,
        reason = "a send that never waits, on the reader thread; bounded by the notes pending in the app"
    )]
    let (tell, heard) = channel::<Heard>();
    let parent = parent();
    let orphaned = Sender::clone(&tell);
    let parent_waited = wake::on_exit(parent, move || drop(orphaned.send(Heard::Orphaned))).is_ok();
    // Kept while the loop runs: the app's folder heard as its entries change.
    let folder = appearing(&tell);
    let folder_waited = folder.is_some();
    // Gone before its end was waited on: there is no one to print to.
    if left(parent) {
        return 0;
    }
    let wait = Waits {
        heard,
        parent,
        parent_waited,
        folder_waited,
    };
    let (mut next, mut told_away) = (0, false);
    loop {
        if let Some((client, conn)) = connected(&tell, &mut next, caller, events) {
            told_away = false;
            on(Step::Connected(&client));
            loop {
                match wait.next(true) {
                    Some(Heard::Note(of, method, params)) if of == conn => {
                        on(Step::Note(&client, &method, &params));
                    }
                    Some(Heard::Closed(of)) if of == conn => break,
                    Some(Heard::Orphaned) | None => return 0,
                    Some(Heard::Note(..) | Heard::Closed(_) | Heard::Appeared) => {}
                }
            }
        }
        if !told_away {
            told_away = true;
            on(Step::Away);
        }
        // Nothing listens: woken when the app's folder changes (its socket made).
        loop {
            match wait.next(false) {
                Some(Heard::Appeared) => break,
                Some(Heard::Orphaned) | None => return 0,
                Some(Heard::Note(..) | Heard::Closed(_)) => {}
            }
        }
    }
}

/// What the loop waits on, and whether each of its causes can be waited for.
struct Waits {
    heard: Receiver<Heard>,
    parent: libc::pid_t,
    parent_waited: bool,
    folder_waited: bool,
}

impl Waits {
    /// The next thing heard. Where the folder cannot be waited on and the app is not reached
    /// (`!connected`), `Appeared` after a while too; where the parent's end cannot be waited
    /// on, `Orphaned` once it is seen gone. None when nothing can be heard any more.
    fn next(&self, connected: bool) -> Option<Heard> {
        let retry = !connected && !self.folder_waited;
        let look = !self.parent_waited;
        loop {
            let most = match (retry, look) {
                (false, false) => return self.heard.recv().ok(),
                (true, _) => RETRY,
                (false, true) => LOOK,
            };
            match self.heard.recv_timeout(most) {
                Ok(heard) => return Some(heard),
                Err(RecvTimeoutError::Disconnected) => return None,
                Err(RecvTimeoutError::Timeout) if look && left(self.parent) => {
                    return Some(Heard::Orphaned);
                }
                Err(RecvTimeoutError::Timeout) if retry => return Some(Heard::Appeared),
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

/// The app reached and subscribed to, its notes told to `tell` under an id of the connection's
/// own (`next` counts them); never started (a follower waits for one). A socket that is there
/// but does not answer yet is the app starting: it is tried a few times more.
fn connected(
    tell: &Sender<Heard>,
    next: &mut u64,
    caller: &Caller,
    events: &[&str],
) -> Option<(Client, u64)> {
    for attempt in 1..=SOON {
        let conn = *next;
        *next += 1;
        let closing = Closing(Sender::clone(tell), conn);
        let hear: Notes = Arc::new(move |method, params| {
            let note = Heard::Note(conn, method.to_string(), params.clone());
            let _ = closing.0.send(note);
        });
        if let Ok(Some(client)) = Client::connect(Start::Never, &hear) {
            // The reader holds the only one left: as it ends, `Closing` says so.
            drop(hear);
            let asked = json!({"events": events});
            let subscribed = client.request("events/subscribe", caller.with(asked));
            return subscribed.is_ok().then_some((client, conn));
        }
        if attempt == SOON || !crate::paths::socket().exists() {
            break;
        }
        crate::backoff(Duration::from_millis(20));
    }
    None
}

/// The app's folder (where its socket is made) watched, `tell` told `Appeared` as an entry of it
/// comes or goes: the files' watch (`crate::files::Watch`), the system's notices. None where it
/// cannot be watched.
fn appearing(tell: &Sender<Heard>) -> Option<crate::files::Watch> {
    let dir = crate::paths::ensure_runtime().ok()?;
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    let appeared = Sender::clone(tell);
    let watch = crate::files::Watch::new(&dir, move |_| drop(appeared.send(Heard::Appeared)));
    let watch = watch.ok()?;
    watch.listen(&std::iter::once(dir).collect());
    Some(watch)
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

#[cfg(test)]
#[path = "follow_tests.rs"]
mod tests;
