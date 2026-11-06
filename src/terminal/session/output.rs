//! The reader's side of a terminal session: the program's output taken into the screen and sent
//! on to the window's tab at a pace, a program's cursor query answered, and how it ended once it
//! has.

use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use super::super::pty;
use super::{PAINT_EVERY, Shared, UNSENT_MAX, nap};

/// How long the reader waits for output while some waits to be sent, and while none does.
const PAINT_POLL_MS: i32 = 33;
const IDLE_POLL_MS: i32 = 1000;

/// The reader thread: output into the screen, frames out to the window, until the terminal
/// closes. It answers a program's cursor-position query, which some ask before they draw.
pub(super) fn read(reader: &std::fs::File, writer: &std::fs::File, shared: &Shared, pid: u32) {
    let mut reader = reader;
    let mut buffer = [0u8; 8192];
    loop {
        let mut gone = false;
        // Output waiting for the pace is sent within a frame; with none, the reader sleeps until
        // the program writes or ends (its end, a kill among them, wakes the poll too).
        let idle = {
            let state = shared.lock();
            state.unsent.is_empty() && !state.replay
        };
        let wait = if idle { IDLE_POLL_MS } else { PAINT_POLL_MS };
        if pty::readable(reader.as_raw_fd(), wait) {
            match reader.read(&mut buffer) {
                Ok(0) => gone = true,
                Ok(n) => {
                    let chunk = buffer.get(..n).unwrap_or_default();
                    take_in(shared, writer, chunk);
                    fronted(shared, reader.as_raw_fd());
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => gone = true,
            }
        }
        if shared.lock().closed {
            return;
        }
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

fn take_in(shared: &Shared, mut writer: &std::fs::File, chunk: &[u8]) {
    let asked = chunk.windows(4).filter(|w| w == b"\x1b[6n").count();
    let mut state = shared.lock();
    let retitled = state.screen.retitled();
    state.screen.feed(chunk);
    let title = (state.screen.retitled() != retitled)
        .then(|| {
            state
                .on_title
                .clone()
                .zip(state.screen.title().map(str::to_string))
        })
        .flatten();
    state.version += 1;
    state.changed_at = Instant::now();
    if state.painter.is_some() && !state.replay {
        state.unsent.extend_from_slice(chunk);
        if state.unsent.len() > UNSENT_MAX {
            state.unsent.clear();
            state.replay = true;
        }
    }
    let (row, column) = state.screen.cursor();
    drop(state);
    if let Some((heard, title)) = title {
        heard(&title);
    }
    for _ in 0..asked {
        let _ = writer.write_all(format!("\x1b[{row};{column}R").as_bytes());
    }
    shared.wake.notify_all();
}

/// How often the name in front is read again while the group stays: a program that replaces
/// itself (`exec`: a shell running `claude` becomes it) keeps its group, not its name.
const RENAMED: Duration = Duration::from_millis(500);

/// Whoever listens told when another program came in front: a program started, or ended, in
/// the terminal, or the one in front became another. The group is read with each output; the
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
                Some((
                    painter,
                    format!("R(\"{}\")", encode(&state.screen.replay())),
                ))
            }
            (true, Some(painter)) if !state.unsent.is_empty() => {
                let output = std::mem::take(&mut state.unsent);
                state.painted_at = Instant::now();
                Some((painter, format!("W(\"{}\")", encode(&output))))
            }
            _ => None,
        }
    };
    if let Some((painter, script)) = due
        && !painter(script)
    {
        shared.lock().painter = None;
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
