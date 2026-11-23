//! What the core says back as it reads a program's output, kept for the session to act on (the
//! core's effects): the answers to a program's queries, which the session writes to the program
//! (the terminal names itself `llxterm <version>`, its terminfo name is the `TERM` its programs
//! are given, and its size is its grid and the cell the view drew); and the steps a shell
//! reports (OSC 133, read in `marks.rs`). The callbacks run inside the core's write, on the thread that feeds it: they
//! only record what they hear, never write to the terminal, and take no lock but their own.

use std::ffi::c_void;
use std::sync::Mutex;

use ghostty_vt_sys as vt;

use super::{Core, Refused, Step, done};

/// The most bytes of answers kept until they are taken: a program that asks again and again in
/// one breath is not reading what it is told.
pub(super) const REPLIES_MOST: usize = 4096;

/// What the terminal calls itself when a program asks (XTVERSION): its name as `TERM_PROGRAM`
/// says it (one word, read up to the space by most who ask), and its version.
const NAME: &str = concat!("llxterm ", env!("CARGO_PKG_VERSION"));

/// The core's record of what it heard, handed to its callbacks as their user data: made with
/// the core and freed after its terminal, so no callback outlives it.
#[derive(Default)]
pub(super) struct Heard {
    said: Mutex<Said>,
    /// A program's copy and its bell (`clip.rs`).
    pub(super) clip: Mutex<super::clip::Clip>,
}

#[derive(Default)]
struct Said {
    step: Option<Step>,
    replies: Vec<u8>,
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

/// The terminal's size for a program that asks it (XTWINOPS, CSI 14, 16, 18 and 19 t): its grid,
/// and its cell in pixels as the view's last resize gave it. Before a view drew it the cell is
/// zero, and the library answers the pixels as zero: a program reads that as not known.
unsafe extern "C" fn measured(
    term: vt::Terminal,
    _userdata: *mut c_void,
    out: *mut vt::SizeReportSize,
) -> bool {
    // SAFETY: the library's size to fill, valid through the call.
    let Some(out) = (unsafe { out.as_mut() }) else {
        return false;
    };
    let cell = super::cell_of(term);
    out.rows = super::get_of(term, vt::TerminalData::ROWS);
    out.columns = super::get_of(term, vt::TerminalData::COLS);
    (out.cell_width, out.cell_height) = (cell.width, cell.height);
    true
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

/// Hands the core its record and has it tell it its answers, its names and its size.
pub(super) fn listen(core: &Core) -> Result<(), Refused> {
    let heard: *const Heard = core.heard;
    // SAFETY: the user data is handed over as the pointer itself; the core only gives it back.
    let given =
        unsafe { vt::ghostty_terminal_set(core.term, vt::TerminalOption::USERDATA, heard.cast()) };
    done(given, "its record of what it heard")?;
    // The terminfo name a program asks (XTGETTCAP `TN`), copied into the terminal.
    let terminfo = vt::String {
        ptr: super::super::TERM.as_ptr(),
        len: super::super::TERM.len(),
    };
    core.set(vt::TerminalOption::TERMINFO_NAME, &terminfo)?;
    let write: vt::TerminalWritePtyFn = Some(wrote);
    let name: vt::TerminalXtversionFn = Some(named);
    let size: vt::TerminalSizeFn = Some(measured);
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
        vt::TerminalOption::SIZE,
        size.map_or(std::ptr::null(), |f| f as *const c_void),
    );
    match set {
        true => Ok(()),
        false => Err(Refused {
            what: "to tell what it heard",
        }),
    }
}

impl Core {
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
}

#[cfg(test)]
#[path = "effects_tests.rs"]
mod tests;
