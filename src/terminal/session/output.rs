//! The reader's side of a terminal session: the program's output taken into the screen and sent
//! on to the window's tab at a pace, a program's cursor query answered, and how it ended once it
//! has.

use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use super::super::pty;
use super::{Input, PAINT_EVERY, Shared, UNSENT_MAX, nap};

/// How long the reader waits for output while some waits to be sent, and while none does.
const PAINT_POLL_MS: i32 = 16;
const IDLE_POLL_MS: i32 = 1000;
/// A title is told on at most this often: a program that sets one all the time is heard now and
/// then, the newest each time.
const TITLE_EVERY: Duration = Duration::from_millis(250);

/// The reader thread: output into the screen, frames out to the window, until the terminal
/// closes. It answers a program's cursor-position query, which some ask before they draw.
pub(super) fn read(reader: &std::fs::File, input: &Input, shared: &Shared, pid: u32) {
    let mut reader = reader;
    let mut buffer = vec![0u8; READ_MOST];
    let mut queries = Queries::default();
    // What follows the output at a pace of its own, not with every kilobyte of a flood: the
    // program in front read again, and whoever waits on the screen woken.
    let mut front = Paced::default();
    let mut woken = Paced::default();
    loop {
        let mut gone = false;
        // Output waiting for the pace (or a title held back) is sent within a frame; with none,
        // the reader sleeps until the program writes or ends (its end, a kill among them, wakes
        // the poll too).
        let idle = {
            let state = shared.lock();
            let sent = state.unsent.is_empty() && !state.replay;
            sent && state.screen.retitled() == state.titled && !front.owed && !woken.owed
        };
        let wait = if idle { IDLE_POLL_MS } else { PAINT_POLL_MS };
        if pty::readable(reader.as_raw_fd(), wait) {
            let (filled, ended) = fill(&mut reader, &mut buffer);
            gone = ended;
            if let Some(chunk) = buffer.get(..filled).filter(|chunk| !chunk.is_empty()) {
                take_in(shared, input, chunk, queries.asked(chunk));
                front.owed = true;
                woken.owed = true;
            }
        }
        if woken.due(WAKE_EVERY) {
            shared.wake.notify_all();
        }
        // The one that floods is the one in front.
        if front.due(FRONT_EVERY) {
            fronted(shared, reader.as_raw_fd());
        }
        if shared.lock().closed {
            return;
        }
        retitled(shared);
        noted(shared);
        paint(shared, Pace::Throttled);
        if gone {
            let status = exit_status(pid);
            let mut state = shared.lock();
            state.ended = true;
            state.status = status;
            let heard = state.on_end.take();
            drop(state);
            paint(shared, Pace::Now);
            paint_ended(shared, status);
            shared.wake.notify_all();
            if let Some(heard) = heard {
                heard();
            }
            return;
        }
    }
}

/// The most output taken in at once. A PTY hands over a kilobyte or a few a read however much
/// waits, so the reads go on until this much is in hand or no more waits: the screen, whoever
/// waits on it and the window are told once for all that waited.
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
/// The most cursor reports one read of output is answered with: a program that asks again and
/// again in one breath is not reading what it is told.
const MOST_CURSOR_REPLIES: usize = 8;

/// A program's cursor-position query.
const QUERY: &[u8] = b"\x1b[6n";

/// The cursor queries in the program's output, read by read: a query split across two reads is
/// found whole, the end of one read kept for the start of the next.
#[derive(Default)]
struct Queries {
    /// The last bytes of the output so far, fewer than a query.
    carried: Vec<u8>,
}

impl Queries {
    /// How many queries end in `chunk`, the output read next.
    fn asked(&mut self, chunk: &[u8]) -> usize {
        self.carried.extend_from_slice(chunk);
        let asked = (self.carried.windows(QUERY.len()))
            .filter(|window| *window == QUERY)
            .count();
        let kept = self.carried.len().saturating_sub(QUERY.len() - 1);
        self.carried.drain(..kept);
        asked
    }
}

fn take_in(shared: &Shared, input: &Input, chunk: &[u8], asked: usize) {
    let mut state = shared.lock();
    let righted = state.screen.feed(chunk);
    state.version += 1;
    state.changed_at = Instant::now();
    if state.painter.is_some() && !state.replay {
        state.unsent.extend_from_slice(chunk);
        state.unsent.extend_from_slice(righted);
        if state.unsent.len() > UNSENT_MAX {
            state.unsent.clear();
            state.replay = true;
        }
    }
    let (row, column) = state.screen.cursor();
    drop(state);
    // The page leaves this query to the session (it answers whether or not a page is there).
    for _ in 0..asked.min(MOST_CURSOR_REPLIES) {
        input.queue(format!("\x1b[{row};{column}R").as_bytes());
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

/// Whoever listens told that the program has another note for the person.
fn noted(shared: &Shared) {
    let heard = {
        let mut state = shared.lock();
        let number = state.screen.notes().waiting().map(|(number, _)| number);
        match number {
            Some(number) if number != state.noted => state.noted = number,
            _ => return,
        }
        state.on_note.clone()
    };
    if let Some(heard) = heard {
        heard();
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
fn fronted(shared: &Shared, master: std::os::fd::RawFd) {
    let group = super::super::foreground::group(master);
    {
        let state = shared.lock();
        let lately = (state.front.read).is_some_and(|read| read.elapsed() < RENAMED);
        if state.front.group == group && lately {
            return;
        }
    }
    let name = group.and_then(super::super::foreground::name);
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

/// What waits for the window sent now, whatever the pace.
pub(super) fn paint_now(shared: &Shared) {
    paint(shared, Pace::Now);
}

/// Whether a send waits for the pace.
#[derive(Clone, Copy)]
enum Pace {
    Throttled,
    Now,
}

/// What a send to the window carries.
enum Frame {
    /// The whole screen, drawn again.
    Screen(Vec<u8>),
    /// The program's output since the last send.
    Output(Vec<u8>),
}

/// The output (or the whole screen) to the window if there is some and it is due; a window that
/// is gone is let go.
fn paint(shared: &Shared, pace: Pace) {
    let due = {
        let mut state = shared.lock();
        let ready = matches!(pace, Pace::Now) || state.painted_at.elapsed() >= PAINT_EVERY;
        match (ready, state.painter.clone()) {
            (true, Some(painter)) if state.replay => {
                state.replay = false;
                state.unsent.clear();
                state.painted_at = Instant::now();
                Some((painter, Frame::Screen(state.screen.replay())))
            }
            (true, Some(painter)) if !state.unsent.is_empty() => {
                state.painted_at = Instant::now();
                Some((painter, Frame::Output(std::mem::take(&mut state.unsent))))
            }
            _ => None,
        }
    };
    // Encoded after the lock is let go: a long frame holds nobody up.
    if let Some((painter, frame)) = due {
        let script = match frame {
            Frame::Screen(bytes) => format!("R(\"{}\")", encode(&bytes)),
            Frame::Output(bytes) => format!("W(\"{}\")", encode(&bytes)),
        };
        if !painter(script) {
            shared.lock().painter = None;
        }
    }
}

fn encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn paint_ended(shared: &Shared, status: Option<i32>) {
    let end = status.map_or_else(|| " · exited".to_string(), |c| format!(" · exited {c}"));
    let painter = shared.lock().painter.clone();
    if let Some(painter) = painter {
        let end = serde_json::to_string(&end).unwrap_or_default();
        painter(format!("E({end})"));
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
