//! What a program asks of the terminal beyond its screen that the app decides on: a copy to the
//! pasteboard (OSC 52, and the other ways a program copies that the core reads the same), kept as
//! its text when it is plain text no longer than `COPIED_MAX`; and the bell (BEL). The callbacks
//! only record, on the thread that feeds the core; the window says where a copy may go and
//! whether a bell is news. A program's read of the pasteboard is never answered: no callback is
//! set for it, so the core tells the program nothing.

use std::ffi::c_void;

use ghostty_vt_sys as vt;

use super::Core;
use super::effects::{heard, set_fn};

/// The most a program puts on the pasteboard at once: more than any selection a person makes in
/// a terminal, and far less than a program could flood the app with.
const COPIED_MAX: usize = 1 << 20;

/// What the callbacks heard since it was last taken.
#[derive(Debug, Default)]
pub(super) struct Clip {
    copied: Option<String>,
    rang: bool,
}

/// The text of `content`, when it is plain text no longer than `COPIED_MAX`.
fn plain(content: &vt::ClipboardContent) -> Option<String> {
    let bytes = |string: vt::String| -> Option<&[u8]> {
        if string.ptr.is_null() {
            return (string.len == 0).then_some(&[][..]);
        }
        // SAFETY: the library's bytes, `len` of them, borrowed through the callback.
        Some(unsafe { std::slice::from_raw_parts(string.ptr, string.len) })
    };
    let mime = bytes(content.mime)?;
    let text = mime.starts_with(b"text/plain") && content.data.len <= COPIED_MAX;
    text.then(|| bytes(content.data))
        .flatten()
        .and_then(|data| String::from_utf8(data.to_vec()).ok())
}

/// A program's copy: its plain text kept for the window, and the program answered at once (a
/// protocol that waits for an answer is told it went, or that it was refused).
unsafe extern "C" fn copied(
    _term: vt::Terminal,
    userdata: *mut c_void,
    write: *const vt::ClipboardWrite,
) {
    // SAFETY: the request is the library's, alive through the callback.
    let Some(write) = (unsafe { write.as_ref() }) else {
        return;
    };
    let contents = match write.contents.is_null() {
        true => &[][..],
        // SAFETY: `contents_len` entries, borrowed through the callback.
        false => unsafe { std::slice::from_raw_parts(write.contents, write.contents_len) },
    };
    let text = contents.iter().find_map(plain);
    let result = match &text {
        Some(_) => vt::ClipboardWriteResult::SUCCESS,
        None => vt::ClipboardWriteResult::DENIED,
    };
    // SAFETY: the user data is the core's `Heard`.
    if let (Some(text), Some(heard)) = (text, unsafe { heard(userdata) }) {
        crate::lock(&heard.clip).copied = Some(text);
    }
    if let Some(reply) = write.reply {
        let answer = vt::ClipboardWriteReply {
            result,
            ..vt::sized!(vt::ClipboardWriteReply)
        };
        // SAFETY: the request's own answer, given once, within the callback.
        unsafe { reply(write, &raw const answer) };
    }
}

/// The program rang the bell.
unsafe extern "C" fn rang(_term: vt::Terminal, userdata: *mut c_void) {
    // SAFETY: the user data is the core's `Heard`.
    if let Some(heard) = unsafe { heard(userdata) } {
        crate::lock(&heard.clip).rang = true;
    }
}

/// Has the core tell the record its program's copies and bells.
pub(super) fn listen(core: &Core) -> Result<(), super::Refused> {
    let copies: vt::TerminalClipboardWriteFn = Some(copied);
    let bells: vt::TerminalBellFn = Some(rang);
    let set = set_fn(
        core,
        vt::TerminalOption::CLIPBOARD_WRITE,
        copies.map_or(std::ptr::null(), |f| f as *const c_void),
    ) && set_fn(
        core,
        vt::TerminalOption::BELL,
        bells.map_or(std::ptr::null(), |f| f as *const c_void),
    );
    match set {
        true => Ok(()),
        false => Err(super::Refused {
            what: "to tell of copies and bells",
        }),
    }
}

impl Core {
    /// What the program asked to put on the pasteboard since this was last asked.
    pub fn program_copy(&mut self) -> Option<String> {
        (self.heard()).and_then(|heard| crate::lock(&heard.clip).copied.take())
    }

    /// Whether the program rang the bell since this was last asked.
    pub fn bell_rang(&mut self) -> bool {
        (self.heard()).is_some_and(|heard| std::mem::take(&mut crate::lock(&heard.clip).rang))
    }
}

#[cfg(test)]
#[path = "clip_tests.rs"]
mod tests;
