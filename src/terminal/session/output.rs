//! The reader's side of a terminal session: the program's output taken into the screen and sent
//! on to the window's tab at a pace, a program's cursor query answered, and how it ended once it
//! has.

use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use super::super::pty;
use super::{PAINT_EVERY, Shared, UNSENT_MAX, nap};

/// The reader thread: output into the screen, frames out to the window, until the terminal
/// closes. It answers a program's cursor-position query, which some ask before they draw.
pub(super) fn read(reader: &std::fs::File, writer: &std::fs::File, shared: &Shared, pid: u32) {
    let mut reader = reader;
    let mut buffer = [0u8; 8192];
    loop {
        let mut gone = false;
        if pty::readable(reader.as_raw_fd(), 33) {
            match reader.read(&mut buffer) {
                Ok(0) => gone = true,
                Ok(n) => {
                    let chunk = buffer.get(..n).unwrap_or_default();
                    take_in(shared, writer, chunk);
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
            drop(state);
            paint(shared, Pace::Now);
            paint_ended(shared, status);
            shared.wake.notify_all();
            return;
        }
    }
}

fn take_in(shared: &Shared, mut writer: &std::fs::File, chunk: &[u8]) {
    let asked = chunk.windows(4).filter(|w| w == b"\x1b[6n").count();
    let mut state = shared.lock();
    state.screen.feed(chunk);
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
    for _ in 0..asked {
        let _ = writer.write_all(format!("\x1b[{row};{column}R").as_bytes());
    }
    shared.wake.notify_all();
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
