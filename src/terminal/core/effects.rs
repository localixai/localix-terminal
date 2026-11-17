//! What the core says back as it reads a program's output, kept for the session to act on (the
//! core's effects): the answers to a program's queries, which the session writes to the program
//! (the terminal names itself `llxterm <version>`); the steps a shell reports (OSC 133, read in
//! `marks.rs`); and a frame a program holds back while it draws the next (synchronized output):
//! the screen as it was when the hold began, for agents, and the view's render state brought up
//! to that moment, so a view keeps drawing the finished frame. The callbacks run inside the core's
//! write, on the thread that feeds it: they only record what they hear, never write to the
//! terminal, and take no lock but their own and the view's render state's, which whoever feeds
//! the core takes after its own.

use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ghostty_vt_sys as vt;

use super::{Core, Refused, Render, Step, cursor_of, done, text};

/// The longest a program's hold on its frame is believed: one that never lets go is let go of
/// after this, as a terminal draws again after a while.
pub(super) const HOLD_MOST: Duration = Duration::from_secs(1);

/// The most bytes of answers kept until they are taken: a program that asks again and again in
/// one breath is not reading what it is told.
pub(super) const REPLIES_MOST: usize = 4096;

/// What the terminal calls itself when a program asks (XTVERSION): its name as `TERM_PROGRAM`
/// says it (one word, read up to the space by most who ask), and its version.
const NAME: &str = concat!("llxterm ", env!("CARGO_PKG_VERSION"));

/// The render state a view draws from, shared by the session that draws it and the core, which
/// brings it up to the moment a hold begins.
pub type Drawn = Arc<Mutex<Option<Render>>>;

/// The core's record of what it heard, handed to its callbacks as their user data: made with
/// the core and freed after its terminal, so no callback outlives it.
#[derive(Default)]
pub(super) struct Heard {
    said: Mutex<Said>,
    drawn: Drawn,
    /// A program's copy and its bell (`clip.rs`).
    pub(super) clip: Mutex<super::clip::Clip>,
}

#[derive(Default)]
struct Said {
    step: Option<Step>,
    replies: Vec<u8>,
    held: Option<Held>,
    /// The core's formatters, plain and joined, to read the screen with as a hold begins.
    formatters: Option<(vt::Formatter, vt::Formatter)>,
}

/// The screen as it was when a program began to hold its frame back, and when that was.
#[derive(Debug, Clone)]
struct Held {
    text: String,
    cursor: (u16, u16),
    search: String,
    since: Instant,
}

/// The core's record behind `userdata`; none when it is null.
///
/// # Safety
/// `userdata` is null or the core's `Heard`, alive through the call.
pub(super) unsafe fn heard<'a>(userdata: *mut c_void) -> Option<&'a Heard> {
    // SAFETY: as the caller says.
    unsafe { userdata.cast::<Heard>().as_ref() }
}

/// An answer the core writes to the program: kept whole while there is room, else dropped whole,
/// since one cut short is a broken one.
unsafe extern "C" fn wrote(
    _term: vt::Terminal,
    userdata: *mut c_void,
    data: *const u8,
    len: usize,
) {
    // SAFETY: the user data is the core's `Heard`.
    let Some(heard) = (unsafe { heard(userdata) }) else {
        return;
    };
    if data.is_null() {
        return;
    }
    // SAFETY: the library's bytes, `len` of them, valid through the call.
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    let mut said = crate::lock(&heard.said);
    if said.replies.len() + bytes.len() <= REPLIES_MOST {
        said.replies.extend_from_slice(bytes);
    }
}

/// The name the terminal answers XTVERSION with; a static string, valid as long as asked.
unsafe extern "C" fn named(_term: vt::Terminal, _userdata: *mut c_void) -> vt::String {
    vt::String {
        ptr: NAME.as_ptr(),
        len: NAME.len(),
    }
}

/// A hold begins or ends. As it begins the terminal holds exactly the frame the program wants
/// left on screen: it is read now, for agents and for the view.
unsafe extern "C" fn hold(term: vt::Terminal, userdata: *mut c_void, held: bool) {
    // SAFETY: the user data is the core's `Heard`.
    let Some(heard) = (unsafe { heard(userdata) }) else {
        return;
    };
    if !held {
        crate::lock(&heard.said).held = None;
        return;
    }
    if let Some(render) = crate::lock(&heard.drawn).as_mut() {
        let _ = render.capture(term);
    }
    let formatters = crate::lock(&heard.said).formatters;
    let Some((plain, joined)) = formatters else {
        return;
    };
    let held = Held {
        text: text::view_text(term, plain),
        cursor: cursor_of(term),
        search: text::view_text(term, joined),
        since: Instant::now(),
    };
    crate::lock(&heard.said).held = Some(held);
}

/// Sets `option` to the callback `callback` (a function's address).
pub(super) fn set_fn(
    core: &Core,
    option: vt::TerminalOption::Type,
    callback: *const c_void,
) -> bool {
    // SAFETY: a callback is handed over as the function itself, of the type the option names.
    let set = unsafe { vt::ghostty_terminal_set(core.term, option, callback) };
    set == vt::Result::SUCCESS
}

/// Hands the core its record and has it tell it its answers, its name and its holds.
pub(super) fn listen(core: &Core) -> Result<(), Refused> {
    let heard: *const Heard = core.heard;
    // SAFETY: the user data is handed over as the pointer itself; the core only gives it back.
    let given =
        unsafe { vt::ghostty_terminal_set(core.term, vt::TerminalOption::USERDATA, heard.cast()) };
    done(given, "its record of what it heard")?;
    if let Some(heard) = core.heard() {
        crate::lock(&heard.said).formatters = Some((core.plain, core.joined));
    }
    let write: vt::TerminalWritePtyFn = Some(wrote);
    let name: vt::TerminalXtversionFn = Some(named);
    let holds: vt::TerminalRenderHoldFn = Some(hold);
    let set = set_fn(
        core,
        vt::TerminalOption::WRITE_PTY,
        write.map_or(std::ptr::null(), |f| f as *const c_void),
    ) && set_fn(
        core,
        vt::TerminalOption::XTVERSION,
        name.map_or(std::ptr::null(), |f| f as *const c_void),
    ) && set_fn(
        core,
        vt::TerminalOption::RENDER_HOLD,
        holds.map_or(std::ptr::null(), |f| f as *const c_void),
    );
    match set {
        true => Ok(()),
        false => Err(Refused {
            what: "to tell what it heard",
        }),
    }
}

impl Core {
    /// The core hears no more holds and forgets its formatters, before they are freed: a hold
    /// the terminal still reports as it goes reads no screen with a formatter that is gone.
    pub(super) fn deafen(&self) {
        let _ = set_fn(self, vt::TerminalOption::RENDER_HOLD, std::ptr::null());
        if let Some(mut said) = self.said() {
            said.formatters = None;
            said.held = None;
        }
    }

    /// The record the callbacks write.
    pub(super) fn heard(&self) -> Option<&Heard> {
        // SAFETY: made in `new` and freed only as the core is dropped.
        unsafe { self.heard.as_ref() }
    }

    fn said(&self) -> Option<std::sync::MutexGuard<'_, Said>> {
        self.heard().map(|heard| crate::lock(&heard.said))
    }

    /// Records the step a shell reported (the semantic prompt callback's part).
    pub(super) fn hear_step(heard: &Heard, step: Step) {
        crate::lock(&heard.said).step = Some(step);
    }

    /// The last step of a command the shell reported; none before its first mark.
    pub fn step(&self) -> Option<Step> {
        self.said().and_then(|said| said.step)
    }

    /// What the core answered the program's queries with since this was last asked, in order:
    /// the session writes it to the program.
    pub fn replies(&mut self) -> Vec<u8> {
        self.said()
            .map(|mut said| std::mem::take(&mut said.replies))
            .unwrap_or_default()
    }

    /// The render state a view draws from, made empty: the session puts one in as a view first
    /// draws and takes it out as the view is hidden.
    pub fn drawn(&self) -> Drawn {
        self.heard()
            .map_or_else(Drawn::default, |heard| Arc::clone(&heard.drawn))
    }

    /// The frame held back, while a program holds it and not past `HOLD_MOST`.
    fn held(&self) -> Option<Held> {
        let said = self.said()?;
        (said.held.clone()).filter(|held| held.since.elapsed() < HOLD_MOST)
    }

    /// Whether a program holds its frame back now (and not past `HOLD_MOST`).
    pub fn holding(&self) -> bool {
        self.held().is_some()
    }

    /// The screen's text and where the cursor was as the hold began, while a program holds.
    pub fn held_text(&self) -> Option<(String, (u16, u16))> {
        self.held().map(|held| (held.text, held.cursor))
    }

    /// The screen's text to look in as the hold began, while a program holds.
    pub fn held_search(&self) -> Option<String> {
        self.held().map(|held| held.search)
    }

    /// How long until a hold the program keeps is let go of (`HOLD_MOST` after it began);
    /// none while it holds nothing.
    pub fn hold_left(&self) -> Option<Duration> {
        let said = self.said()?;
        (said.held.as_ref()).map(|held| HOLD_MOST.saturating_sub(held.since.elapsed()))
    }

    /// Lets go of a hold kept past `HOLD_MOST`, as the program would have; whether there was one.
    pub fn let_go_if_long(&mut self) -> bool {
        let long = self.said().is_some_and(|said| {
            (said.held.as_ref()).is_some_and(|held| held.since.elapsed() >= HOLD_MOST)
        });
        if long {
            let off = vt::TerminalModeConfig {
                mode: 2026,
                value: false,
            };
            let _ = self.set(vt::TerminalOption::MODE, &off);
            if let Some(mut said) = self.said() {
                said.held = None;
            }
        }
        long
    }
}

#[cfg(test)]
#[path = "effects_tests.rs"]
mod tests;
