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
    /// Called once, when the program ends: who keeps the session hears it.
    on_end: Option<Box<dyn FnOnce() + Send>>,
    /// Called with the program's title each time it changes.
    on_title: Option<Arc<Titled>>,
    /// The program in front when last read (the terminal's foreground group, its name, and
    /// when), and who hears it each time it changes (its name; none when it is not known).
    front: Front,
    on_front: Option<Arc<Fronted>>,
}

/// Hears the program in front of a terminal, by the name it was started as.
type Fronted = dyn Fn(Option<&str>) + Send + Sync;

/// The program in front of a terminal, as last read.
#[derive(Debug, Default, Clone)]
struct Front {
    group: Option<libc::pid_t>,
    name: Option<String>,
    read: Option<Instant>,
}

impl Front {
    fn read(group: Option<libc::pid_t>, name: Option<String>) -> Self {
        Self {
            group,
            name,
            read: Some(Instant::now()),
        }
    }
}

/// Hears the title a program sets, cleaned to words (see `screen`).
type Titled = dyn Fn(&str) + Send + Sync;

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
        Self::start_after(command, dir, vars, painter, "")
    }

    /// `start`, its screen showing `before` first, dimmed (the last screen of the terminal it
    /// takes the place of, restored): the program's output follows under it.
    ///
    /// # Errors
    /// As `start`.
    pub fn start_after(
        command: &str,
        dir: &Path,
        vars: Option<&[(String, String)]>,
        painter: Option<Painter>,
        before: &str,
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
                on_end: None,
                on_title: None,
                front: Front::default(),
                on_front: None,
            }),
            wake: Condvar::new(),
        });
        let session = Self {
            command: command.to_string(),
            shared: Arc::clone(&shared),
            writer: Mutex::new(master.try_clone()?),
            child: Mutex::new(child),
        };
        // Before the reader starts: the program's first output comes after it.
        if !before.trim().is_empty() {
            shared.lock().screen.feed(&restored(before));
        }
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
        // Now, not when the reader next wakes: an idle one sleeps a while.
        output::paint_now(&self.shared);
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

    /// What the screen shows, as text: kept when the terminal closes, shown again when it is
    /// restored.
    pub fn text(&self) -> String {
        self.shared.lock().screen.text().0
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

    /// How the program ended, once it has: its exit code (128 and the signal), when known.
    pub fn status(&self) -> Option<i32> {
        self.shared.lock().status
    }

    /// `heard` once the program ends: now, if it has.
    pub fn on_end(&self, heard: impl FnOnce() + Send + 'static) {
        let mut state = self.shared.lock();
        if state.ended {
            drop(state);
            heard();
        } else {
            state.on_end = Some(Box::new(heard));
        }
    }

    /// The program in front in the terminal now (the one the person typed in its shell, else
    /// its own), by the name it was started as.
    pub fn foreground(&self) -> Option<String> {
        use std::os::fd::AsRawFd;
        super::foreground::of(crate::lock(&self.writer).as_raw_fd())
    }

    /// The folder the program in front is in now (the shell's after a `cd`), if it is known.
    pub fn folder(&self) -> Option<std::path::PathBuf> {
        use std::os::fd::AsRawFd;
        let group = super::foreground::group(crate::lock(&self.writer).as_raw_fd())?;
        super::foreground::cwd(group)
    }

    /// `heard` with the program in front each time another comes in front (a program the
    /// person typed in its shell, or the shell again): now, with the one in front.
    pub fn on_front(&self, heard: impl Fn(Option<&str>) + Send + Sync + 'static) {
        use std::os::fd::AsRawFd;
        let heard: Arc<Fronted> = Arc::new(heard);
        let group = super::foreground::group(crate::lock(&self.writer).as_raw_fd());
        let name = group.and_then(super::foreground::name);
        {
            let mut state = self.shared.lock();
            state.on_front = Some(Arc::clone(&heard));
            state.front = Front::read(group, name.clone());
        }
        heard(name.as_deref());
    }

    /// The title the program set last, as words.
    pub fn title(&self) -> Option<String> {
        self.shared.lock().screen.title().map(str::to_string)
    }

    /// `heard` with the program's title each time it changes: now, if it set one already.
    pub fn on_title(&self, heard: impl Fn(&str) + Send + Sync + 'static) {
        let heard: Arc<Titled> = Arc::new(heard);
        let now = {
            let mut state = self.shared.lock();
            state.on_title = Some(Arc::clone(&heard));
            state.screen.title().map(str::to_string)
        };
        if let Some(title) = now {
            heard(&title);
        }
    }

    /// End the program and its tab, once: a second close would signal a process group whose
    /// leader was reaped, an id the system may have given to another by then.
    pub fn close(&self) {
        let painter = {
            let mut state = self.shared.lock();
            if std::mem::replace(&mut state.closed, true) {
                return;
            }
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

/// `before` as a terminal shows a screen restored: its lines dimmed, a line break after.
fn restored(before: &str) -> Vec<u8> {
    let lines: Vec<&str> = before.lines().collect();
    format!("\x1b[2m{}\x1b[0m\r\n", lines.join("\r\n")).into_bytes()
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
