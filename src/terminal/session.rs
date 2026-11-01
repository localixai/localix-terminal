//! A terminal session: a program in a PTY whose screen is kept as a terminal would show it. A
//! reader thread feeds the screen and paints it into the window's tab when one is attached;
//! calls type into it and wait for its output to settle or for a text to show.

use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::Path;
use std::process::Child;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::pty::{self, Pty};

mod output;
use super::screen::Screen;
use super::{COLUMNS, ROWS, keys};

/// Draws a script into the window's page (`W(..)` output, `R(..)` the screen again, `E(..)` the
/// end); false once the tab is gone.
pub type Painter = Box<dyn Fn(String) -> bool + Send + Sync>;

/// Output is sent to the window at most this often.
const PAINT_EVERY: Duration = Duration::from_millis(33);
/// The most output kept for a window that does not keep up: past it the screen is sent again.
const UNSENT_MAX: usize = 1 << 20;
/// Output that stays still this long has settled.
const SETTLE: Duration = Duration::from_millis(400);
/// A wait with nothing changing ends after this much, a program that prints nothing being quiet.
const QUIET: Duration = Duration::from_millis(1500);
/// The longest a call waits when it names no limit.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(10);

struct State {
    screen: Screen,
    /// Counts the chunks taken in, so a wait knows something changed.
    version: u64,
    changed_at: Instant,
    ended: bool,
    /// How the program ended, once it has and it is known.
    status: Option<i32>,
    painter: Option<Arc<Painter>>,
    painted_at: Instant,
    /// Output the window has not been sent yet, as the program wrote it.
    unsent: Vec<u8>,
    /// The window is to be sent the whole screen again, not the output since.
    replay: bool,
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        crate::lock(&self.state)
    }
}

/// One terminal session.
pub struct Session {
    command: String,
    shared: Arc<Shared>,
    writer: Mutex<std::fs::File>,
    child: Mutex<Child>,
}

/// How a wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    Matched,
    Settled,
    Deadline,
    Exited,
}

impl Session {
    /// Start `command` in `dir`; `painter` shows it in a window.
    pub fn start(
        command: &str,
        dir: &Path,
        vars: Option<&[(String, String)]>,
        painter: Option<Painter>,
    ) -> io::Result<Self> {
        let Pty { child, master } = pty::spawn(command, dir, vars)?;
        let reader = master.try_clone()?;
        let now = Instant::now();
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                screen: Screen::new(ROWS, COLUMNS),
                version: 0,
                changed_at: now,
                ended: false,
                status: None,
                painter: painter.map(Arc::new),
                painted_at: now,
                unsent: Vec::new(),
                replay: false,
                closed: false,
            }),
            wake: Condvar::new(),
        });
        let session = Self {
            command: command.to_string(),
            shared: Arc::clone(&shared),
            writer: Mutex::new(master.try_clone()?),
            child: Mutex::new(child),
        };
        let pid = crate::lock(&session.child).id();
        let writer = master;
        std::thread::Builder::new()
            .name("desk-terminal".into())
            .spawn(move || output::read(&reader, &writer, &shared, pid))?;
        Ok(session)
    }

    /// Type `input` (a line break is Enter), then press `keys` one at a time.
    pub fn type_in(&self, input: &str, names: &[String]) -> Result<(), String> {
        let mut bytes = keys::typed(input);
        for name in names {
            let cursor = self.shared.lock().screen.application_cursor();
            let key = keys::encode(name, cursor)
                .ok_or_else(|| format!("no key called `{name}`: {}", keys::NAMES))?;
            if !bytes.is_empty() && !input.is_empty() {
                // Text followed at once by Enter is taken for a paste.
                self.write(&std::mem::take(&mut bytes))?;
                nap(30);
            }
            bytes.extend(key);
        }
        self.write(&bytes)
    }

    /// The window's page is new, `columns` by `rows`: the terminal takes that size and the page
    /// is sent the whole screen.
    pub fn loaded(&self, columns: u16, rows: u16) {
        self.resized(columns, rows);
        self.shared.lock().replay = true;
    }

    /// The window's pane changed to `columns` by `rows`: the program is told, as at a terminal.
    pub fn resized(&self, columns: u16, rows: u16) {
        if columns == 0 || rows == 0 {
            return;
        }
        let _ = pty::resize(&crate::lock(&self.writer), columns, rows);
        self.shared.lock().screen.resize(rows, columns);
    }

    /// What the person typed in the window, as the terminal sends it.
    pub fn typed(&self, text: &str) {
        let cursor = self.shared.lock().screen.application_cursor();
        let _ = self.write(&keys::from_window(text, cursor));
    }

    fn write(&self, bytes: &[u8]) -> Result<(), String> {
        let mut writer = crate::lock(&self.writer);
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut sent = 0;
        while sent < bytes.len() {
            match writer.write(bytes.get(sent..).unwrap_or_default()) {
                Ok(n) => sent += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                    nap(5);
                }
                Err(e) => return Err(format!("the terminal takes no input ({e})")),
            }
        }
        Ok(())
    }

    /// How many times `needle` shows on the screen now.
    pub fn count(&self, needle: &str) -> usize {
        match needle.is_empty() {
            true => 0,
            false => self.shared.lock().screen.text().0.matches(needle).count(),
        }
    }

    /// Wait for `needle` to show more than `before` times, for the output to settle, or for the
    /// program to end; at most `most`.
    pub fn wait(&self, needle: Option<&str>, before: usize, most: Duration) -> Until {
        let needle = needle.filter(|needle| !needle.is_empty());
        let start = Instant::now();
        let mut state = self.shared.lock();
        let first = state.version;
        loop {
            if needle.is_some_and(|n| state.screen.text().0.matches(n).count() > before) {
                return Until::Matched;
            }
            if state.ended {
                return Until::Exited;
            }
            let idle = state.changed_at.elapsed();
            let moved = state.version != first;
            if needle.is_none() && ((moved && idle >= SETTLE) || start.elapsed() >= QUIET) {
                return Until::Settled;
            }
            let left = most.saturating_sub(start.elapsed());
            if left.is_zero() {
                return Until::Deadline;
            }
            let slice = left.min(Duration::from_millis(50));
            state = crate::wait_timeout(&self.shared.wake, state, slice);
        }
    }

    /// The screen as the model reads it, and how the program ended if it did.
    pub fn snapshot(&self) -> String {
        let state = self.shared.lock();
        let (text, (row, column)) = state.screen.text();
        let (rows, columns) = state.screen.size();
        let mut out = format!("screen {columns}×{rows}, cursor at row {row} col {column}:\n{text}");
        match (state.ended, state.status) {
            (true, Some(code)) => {
                let _ = write!(out, "\n(the program exited with {code})");
            }
            (true, None) => out.push_str("\n(the program ended)"),
            (false, _) => {}
        }
        out
    }

    /// How many chunks of output the screen has taken in.
    pub fn version(&self) -> u64 {
        self.shared.lock().version
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn ended(&self) -> bool {
        self.shared.lock().ended
    }

    /// End the program and its tab.
    pub fn close(&self) {
        let painter = {
            let mut state = self.shared.lock();
            state.closed = true;
            state.painter.take()
        };
        drop(painter);
        pty::kill(&mut crate::lock(&self.child));
        self.shared.wake.notify_all();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "a short pause with no event to wait on: between a paste and its Enter, and while a full terminal drains"
)]
fn nap(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
