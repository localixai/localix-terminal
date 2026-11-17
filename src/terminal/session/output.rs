//! The reader's side of a terminal session: the program's output taken into the screen, what
//! the core answered a program's queries with written back to it (and the session's answers to
//! its status queries), and how it ended once it has.

use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use super::super::pty;
use super::{Input, Shared, nap};

/// How long the reader waits for output while something waits to be told, and while nothing does.
const BUSY_POLL_MS: i32 = 16;
const IDLE_POLL_MS: i32 = 1000;
/// A title is told on at most this often: a program that sets one all the time is heard now and
/// then, the newest each time.
const TITLE_EVERY: Duration = Duration::from_millis(250);

/// The reader thread: output into the screen, until the terminal closes; what the core answers a
/// program's queries with (where the cursor is, which some ask before they draw; which terminal
/// it is) is written back to it, with or without a view.
pub(super) fn read(reader: &std::fs::File, input: &Input, shared: &Shared, pid: u32) {
    let mut reader = reader;
    let mut buffer = vec![0u8; READ_MOST];
    // What follows the output at a pace of its own, not with every kilobyte of a flood: the
    // program in front read again, and whoever waits on the screen woken.
    let mut front = Paced::default();
    let mut woken = Paced::default();
    loop {
        let mut gone = false;
        // A title held back or a wake owed is told within a frame; with none, the reader sleeps
        // until the program writes or ends (its end, a kill among them, wakes the poll too).
        // While a program holds its frame back, the wait ends when the hold's second does: the
        // one deadline there is, and only while a frame is held (`held_out`).
        let (idle, hold) = {
            let state = shared.lock();
            let idle = state.screen.retitled() == state.titled && !front.owed && !woken.owed;
            (idle, state.screen.core().hold_left())
        };
        let wait = if idle { IDLE_POLL_MS } else { BUSY_POLL_MS };
        let wait = hold.map_or(wait, |left| wait.min(millis_past(left)));
        if pty::readable(reader.as_raw_fd(), wait) {
            let (filled, ended) = fill(&mut reader, &mut buffer);
            gone = ended;
            if let Some(chunk) = buffer.get(..filled).filter(|chunk| !chunk.is_empty()) {
                take_in(shared, input, chunk);
                front.owed = true;
                woken.owed = true;
            }
        }
        held_out(shared);
        if woken.due(WAKE_EVERY) {
            shared.wake.notify_all();
            changed(shared);
        }
        // The one that floods is the one in front.
        if front.due(FRONT_EVERY) {
            fronted(shared, reader.as_raw_fd(), pid);
        }
        if shared.lock().closed {
            drain_closed(&mut reader, &mut buffer, shared);
            return;
        }
        retitled(shared);
        super::told::noted(shared);
        super::told::commanded(shared);
        super::status::statused(shared);
        if gone {
            let status = exit_status(pid);
            let mut state = shared.lock();
            state.ended = true;
            state.status = status;
            // What the program left at work goes with it.
            state.screen.status_mut().drop_transient();
            let heard = state.on_end.take();
            drop(state);
            super::status::statused(shared);
            shared.wake.notify_all();
            changed(shared);
            if let Some(heard) = heard {
                heard();
            }
            return;
        }
    }
}

/// After the close, what the program still has in its terminal is read and dropped until the
/// close has reaped it (`ended`) or its end of the terminal is closed. The last close of a
/// terminal's program end waits, even in a killed program's exit, until all that was written
/// into it is read (macOS): with nobody reading, the program would never finish exiting, and
/// the close that waits for it would never return.
fn drain_closed(reader: &mut &std::fs::File, buffer: &mut [u8], shared: &Shared) {
    loop {
        if pty::readable(reader.as_raw_fd(), BUSY_POLL_MS) && fill(reader, buffer).1 {
            return;
        }
        if shared.lock().ended {
            return;
        }
    }
}

/// The most output taken in at once. A PTY hands over a kilobyte or a few a read however much
/// waits, so the reads go on until this much is in hand or no more waits: the screen and whoever
/// waits on it are told once for all that waited.
const READ_MOST: usize = 1 << 16;
/// Reads into `buffer` until it is full or nothing more waits. How much was read, and whether the
/// program is gone (its end of the terminal closed).
fn fill(reader: &mut impl Read, buffer: &mut [u8]) -> (usize, bool) {
    let mut filled = 0;
    while let Some(room) = buffer.get_mut(filled..).filter(|room| !room.is_empty()) {
        match reader.read(room) {
            Ok(0) => return (filled, true),
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => return (filled, true),
        }
    }
    (filled, false)
}
fn take_in(shared: &Shared, input: &Input, chunk: &[u8]) {
    let mut state = shared.lock();
    // Recorded as the program wrote it, before the screen filters anything out.
    super::record::keep(&mut state.record, |record| record.output(chunk));
    state.screen.feed(chunk);
    state.version += 1;
    state.changed_at = Instant::now();
    let replies = state.screen.replies();
    let status_asked = state.screen.status_asked();
    drop(state);
    // The core's answers, written after the lock is let go of.
    if !replies.is_empty() {
        input.queue(&replies);
    }
    super::status::answer(input, status_asked);
}

/// A frame a program held past its second let go of, as the program would have: whoever
/// waits and the view are told at once, so its newest frame is drawn with nothing more written.
fn held_out(shared: &Shared) {
    if shared.lock().screen.core_mut().let_go_if_long() {
        shared.wake.notify_all();
        changed(shared);
    }
}

/// `left` in whole milliseconds, rounded up, as a poll waits.
fn millis_past(left: Duration) -> i32 {
    i32::try_from(left.as_millis())
        .unwrap_or(i32::MAX)
        .saturating_add(1)
}

/// The view told the screen changed, outside the lock.
fn changed(shared: &Shared) {
    let heard = shared.lock().on_changed.clone();
    if let Some(heard) = heard {
        heard();
    }
}

/// Whoever listens told the program's title, if it changed and the last one went out long enough
/// ago.
fn retitled(shared: &Shared) {
    let due = {
        let mut state = shared.lock();
        let now = state.screen.retitled();
        if now == state.titled || state.titled_at.elapsed() < TITLE_EVERY {
            return;
        }
        state.titled = now;
        state.titled_at = Instant::now();
        state
            .on_title
            .clone()
            .zip(state.screen.title().map(str::to_string))
    };
    if let Some((heard, title)) = due {
        heard(&title);
    }
}

/// The program in front is read at most this often while output keeps coming.
const FRONT_EVERY: Duration = Duration::from_millis(50);
/// Whoever waits on the screen is woken at most this often while output keeps coming: a wait
/// looks the screen through each time, and woken with every kilobyte it held a flood to a
/// quarter of its speed. A text is still found within a frame of showing.
const WAKE_EVERY: Duration = Duration::from_millis(16);
/// Something done after output, at most so often: at once when the last time is long past, else
/// owed until its time comes (the reader does not sleep long while anything is owed).
#[derive(Default)]
struct Paced {
    at: Option<Instant>,
    owed: bool,
}
impl Paced {
    /// Whether it is owed and its time has come; then it is no longer owed.
    fn due(&mut self, every: Duration) -> bool {
        if !self.owed || self.at.is_some_and(|at| at.elapsed() < every) {
            return false;
        }
        self.at = Some(Instant::now());
        self.owed = false;
        true
    }
}
/// How often the name in front is read again while the group stays: a program that replaces
/// itself (`exec`: a shell running `claude` becomes it) keeps its group, not its name.
const RENAMED: Duration = Duration::from_millis(500);

/// Whoever listens told when another program came in front: a program started, or ended, in
/// the terminal, or the one in front became another. The group is read with the output (`FRONT_EVERY`); the
/// name when it changed, or now and then while it stays.
fn fronted(shared: &Shared, master: std::os::fd::RawFd, pid: u32) {
    let group = super::super::foreground::group(master);
    {
        let state = shared.lock();
        let lately = (state.front.read).is_some_and(|read| read.elapsed() < RENAMED);
        if state.front.group == group && lately {
            return;
        }
    }
    let name = group.and_then(super::super::foreground::name);
    super::status::fronted(shared, group, pid);
    let heard = {
        let mut state = shared.lock();
        let same = state.front.group == group && state.front.name == name;
        state.front = super::Front::read(group, name.clone());
        if same {
            return;
        }
        state.on_front.clone()
    };
    if let Some(heard) = heard {
        heard(name.as_deref());
    }
}

/// How `pid` ended: its exit code, or 128 and the signal; looked at for a moment, not reaped.
fn exit_status(pid: u32) -> Option<i32> {
    let deadline = Instant::now() + Duration::from_millis(300);
    loop {
        // SAFETY: siginfo_t is a plain C struct, for which all zeros is a valid value.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let flags = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
        // SAFETY: waitid only writes into `info`, which lives through the call.
        let done = unsafe { libc::waitid(libc::P_PID, libc::id_t::from(pid), &mut info, flags) };
        if done != 0 {
            return None;
        }
        if info.si_signo == libc::SIGCHLD {
            // SAFETY: a SIGCHLD's info carries the status.
            let code = unsafe { info.si_status() };
            return Some(match info.si_code {
                libc::CLD_EXITED => code,
                _ => 128 + code,
            });
        }
        if Instant::now() >= deadline {
            return None;
        }
        nap(5);
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
