//! A terminal session: a program in a PTY whose screen is kept as a terminal would show it. A
//! reader thread feeds the screen and paints it into the window's tab when one is attached;
//! calls type into it and wait for its output to settle or for a text to show.

use std::io;
use std::path::Path;
use std::process::Child;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::pty::{self, Pty};

mod input;
mod output;
mod read;
mod wait;
use self::input::Input;
use super::screen::Screen;
use super::{COLUMNS, ROWS, keys};

/// Draws a script into the window's page (`W(..)` output, `R(..)` the screen again, `E(..)` the
/// end); false once the tab is gone.
pub type Painter = Box<dyn Fn(String) -> bool + Send + Sync>;

/// Output is sent to the window at most this often.
const PAINT_EVERY: Duration = Duration::from_millis(16);
/// The most output kept for a window that does not keep up: past it the screen is sent again.
const UNSENT_MAX: usize = 1 << 20;
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
    /// How many commands the shell had ended when the last input was typed: the wait after it
    /// counts from there, so a command that ends before the wait begins is not missed.
    typed_at: Option<u64>,
    closed: bool,
    /// Called once, when the program ends: who keeps the session hears it.
    on_end: Option<Box<dyn FnOnce() + Send>>,
    /// Called with the program's title each time it changes.
    on_title: Option<Arc<Titled>>,
    /// The count of title changes (`Screen::retitled`) `on_title` last heard, and when: a title
    /// that changes again at once is told once the next is due.
    titled: u32,
    titled_at: Instant,
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
    /// What is typed: written to the PTY by a thread of its own.
    input: Arc<Input>,
    /// The PTY for what is not a write (its size, its foreground group): a write that waits on a
    /// program that is not reading never holds it.
    control: std::fs::File,
    child: Mutex<Child>,
}

/// How a wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    Matched,
    Settled,
    /// The time ran out; with no text named, the output was still changing after `QUIET`.
    Deadline,
    Exited,
    /// The shell said the command typed into it ended: with what code, after how long.
    Finished(super::marks::Ended),
    /// The time ran out with the shell's command still at work.
    Running,
    /// The program reads a line it does not show (a password): it goes on only once that is
    /// typed, so the wait is not sat out.
    Asking,
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
        let Pty { mut child, master } = pty::spawn(command, dir, vars)?;
        let ends = master
            .try_clone()
            .and_then(|reader| Ok((reader, master.try_clone()?)));
        let (reader, control) = match ends {
            Ok(ends) => ends,
            Err(why) => {
                // Started, with no end left to keep it by: not left running with no tab.
                pty::kill(&mut child);
                return Err(why);
            }
        };
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
                typed_at: None,
                closed: false,
                on_end: None,
                on_title: None,
                titled: 0,
                titled_at: now,
                front: Front::default(),
                on_front: None,
            }),
            wake: Condvar::new(),
        });
        // From here a failure drops the session, which ends the program.
        let session = Self {
            command: command.to_string(),
            shared: Arc::clone(&shared),
            input: Arc::new(Input::default()),
            control,
            child: Mutex::new(child),
        };
        // Before the reader starts: the program's first output comes after it.
        if !before.trim().is_empty() {
            shared.lock().screen.feed(&restored(before));
        }
        let pid = crate::lock(&session.child).id();
        let input = Arc::clone(&session.input);
        std::thread::Builder::new()
            .name("desk-terminal-input".into())
            .spawn(move || input.drain(master))?;
        let input = Arc::clone(&session.input);
        std::thread::Builder::new()
            .name("desk-terminal".into())
            .spawn(move || output::read(&reader, &input, &shared, pid))?;
        Ok(session)
    }

    /// Type `input` (a line break is Enter; lines are one paste for a program that takes
    /// pastes), then press `keys` one at a time. A key that does not
    /// exist is refused before anything is typed.
    pub fn type_in(&self, input: &str, names: &[String]) -> Result<(), String> {
        let (cursor, pastes) = {
            let mut state = self.shared.lock();
            state.typed_at = Some(state.screen.marks().ended().0);
            (
                state.screen.application_cursor(),
                state.screen.bracketed_paste(),
            )
        };
        let mut pressed = Vec::new();
        for name in names {
            pressed.extend(
                keys::encode(name, cursor)
                    .ok_or_else(|| format!("no key called `{name}`: {}", keys::NAMES))?,
            );
        }
        let (mut bytes, mut after) = keys::typed(input, pastes);
        after.extend(pressed);
        if bytes.is_empty() || after.is_empty() {
            bytes.extend(after);
            return self.input.send(&bytes);
        }
        // Text followed at once by Enter is taken for a paste.
        self.input.send(&bytes)?;
        nap(30);
        self.input.send(&after)
    }

    /// The window's page is new, `columns` by `rows`: the terminal takes that size and the page
    /// is sent the whole screen.
    pub fn loaded(&self, columns: u16, rows: u16) {
        self.resized(columns, rows);
        self.behind();
    }

    /// The window's page is behind the screen (it is new, or dropped output): it is sent the whole
    /// screen again.
    pub fn behind(&self) {
        self.shared.lock().replay = true;
        // Now, not when the reader next wakes: an idle one sleeps a while.
        output::paint_now(&self.shared);
    }

    /// The window's pane changed to `columns` by `rows`: the program is told, as at a terminal.
    pub fn resized(&self, columns: u16, rows: u16) {
        if columns == 0 || rows == 0 {
            return;
        }
        let _ = pty::resize(&self.control, columns, rows);
        self.shared.lock().screen.resize(rows, columns);
    }

    /// What the person typed in the window, as the terminal sends it: queued, the window never
    /// waits for the program to read it.
    pub fn typed(&self, text: &str) {
        let cursor = self.shared.lock().screen.application_cursor();
        self.input.queue(&keys::from_window(text, cursor));
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
    #[cfg(test)]
    pub fn foreground(&self) -> Option<String> {
        use std::os::fd::AsRawFd;
        super::foreground::of(self.control.as_raw_fd())
    }

    /// The process of the program the terminal was opened with: what runs in the terminal is it,
    /// or descends from it.
    pub fn pid(&self) -> u32 {
        crate::lock(&self.child).id()
    }

    /// `heard` with the program in front each time another comes in front (a program the
    /// person typed in its shell, or the shell again): now, with the one in front.
    pub fn on_front(&self, heard: impl Fn(Option<&str>) + Send + Sync + 'static) {
        use std::os::fd::AsRawFd;
        let heard: Arc<Fronted> = Arc::new(heard);
        let group = super::foreground::group(self.control.as_raw_fd());
        let name = group.and_then(super::foreground::name);
        {
            let mut state = self.shared.lock();
            state.on_front = Some(Arc::clone(&heard));
            state.front = Front::read(group, name.clone());
        }
        heard(name.as_deref());
    }

    /// `heard` with the program's title each time it changes: now, if it set one already.
    pub fn on_title(&self, heard: impl Fn(&str) + Send + Sync + 'static) {
        let heard: Arc<Titled> = Arc::new(heard);
        let now = {
            let mut state = self.shared.lock();
            state.on_title = Some(Arc::clone(&heard));
            state.titled = state.screen.retitled();
            state.titled_at = Instant::now();
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
        self.input.close();
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
