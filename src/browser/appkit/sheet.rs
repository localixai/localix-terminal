//! The person's questions, asked as the system asks them (macOS): an alert, a folder to open. On
//! the window they are a sheet, and the window's loop runs on while it waits (pages draw, agents
//! are answered); when the window is away (put away, in the Dock) they are an alert of their own,
//! in front of every app, that holds the loop until it is answered.

use std::ffi::CStr;
use std::sync::Mutex;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};

use super::{activate, string, text};

/// Whether a question hangs from the window as a sheet: only while it is on screen.
fn hangs(visible: bool, miniaturized: bool) -> bool {
    visible && !miniaturized
}

/// `done`, callable more than once, run the first time only: the system's completion handler is a
/// block it may keep, and the answer is given once.
fn once<T>(done: impl FnOnce(T)) -> impl Fn(T) {
    let done = Mutex::new(Some(done));
    move |answer| {
        let done = crate::lock(&done).take();
        if let Some(done) = done {
            done(answer);
        }
    }
}

/// `asking` (an NSAlert or an NSOpenPanel, which take the same messages) run on `window` as a
/// sheet when it is on screen, else on its own; `done` takes the response once it is given.
fn ask(
    asking: &AnyObject,
    window: Option<&tao::window::Window>,
    done: impl FnOnce(isize) + 'static,
) {
    activate();
    let window = window.map(super::native).filter(|window| {
        // SAFETY: a live NSWindow (borrowed) answers whether it is on screen, a BOOL.
        let visible: Bool = unsafe { msg_send![*window, isVisible] };
        // SAFETY: as above: whether it is in the Dock.
        let docked: Bool = unsafe { msg_send![*window, isMiniaturized] };
        hangs(visible.as_bool(), docked.as_bool())
    });
    let Some(window) = window else {
        // SAFETY: on the main thread, as the event loop is: it runs until it is answered.
        let answer: isize = unsafe { msg_send![asking, runModal] };
        return done(answer);
    };
    let block = RcBlock::new(once(done));
    // SAFETY: an alert or a panel takes a live NSWindow and a block of `void (^)(NSModalResponse)`,
    // which it copies and calls once, on the main thread, when the person answers.
    let _: () =
        unsafe { msg_send![asking, beginSheetModalForWindow: window, completionHandler: &*block] };
}

/// Whoever waits for an answer to a question.
type Waiting = Vec<Box<dyn FnOnce(bool)>>;

/// The questions up, each with whoever waits for its answer: one asked again while it is up (Quit
/// pressed twice) waits for the one shown, and never stacks a second sheet under it.
pub(super) struct Up<Q> {
    asked: Vec<(Q, Waiting)>,
}

impl<Q> Default for Up<Q> {
    fn default() -> Self {
        Self { asked: Vec::new() }
    }
}

impl<Q: PartialEq> Up<Q> {
    /// `done` waits for `question`'s answer; whether it is to be shown (it was not up).
    pub(super) fn join(&mut self, question: Q, done: Box<dyn FnOnce(bool)>) -> bool {
        if let Some((_, waiting)) = self.asked.iter_mut().find(|(up, _)| *up == question) {
            waiting.push(done);
            return false;
        }
        self.asked.push((question, vec![done]));
        true
    }

    /// `question` answered: it is no longer up, and whoever waited for it is handed back, to be
    /// told.
    pub(super) fn answered(&mut self, question: &Q) -> Waiting {
        let at = self.asked.iter().position(|(up, _)| up == question);
        at.map(|at| self.asked.remove(at).1).unwrap_or_default()
    }
}

thread_local! {
    /// The questions up in the app: asked and answered on the main thread alone.
    static UP: Mutex<Up<super::super::Confirm>> = Mutex::new(Up::default());
}

/// `question` asked of the person on `window` (or in front of every app, when it is away); `done`
/// takes whether they chose `yes` over Cancel. Asked again while it is up, it is not shown again:
/// `done` takes the answer to the one shown.
pub(in super::super) fn confirm(
    question: &super::super::Confirm,
    window: Option<&tao::window::Window>,
    done: impl FnOnce(bool) + 'static,
) {
    /// NSAlertFirstButtonReturn: the first button added, `yes`.
    const FIRST: isize = 1000;
    if !UP.with(|up| crate::lock(up).join(question.clone(), Box::new(done))) {
        return;
    }
    // SAFETY: `new` on NSAlert answers a retained alert.
    let alert: Retained<AnyObject> = unsafe { msg_send![class!(NSAlert), new] };
    if let Some(title) = string(&question.title) {
        // SAFETY: an alert takes its message, an NSString.
        let _: () = unsafe { msg_send![&*alert, setMessageText: &*title] };
    }
    if let Some(detail) = string(&question.detail) {
        // SAFETY: an alert takes its informative text, an NSString.
        let _: () = unsafe { msg_send![&*alert, setInformativeText: &*detail] };
    }
    for button in [question.yes.as_str(), "Cancel"] {
        if let Some(title) = string(button) {
            // SAFETY: an alert takes a button's title, an NSString, and answers the button.
            let _: *mut AnyObject = unsafe { msg_send![&*alert, addButtonWithTitle: &*title] };
        }
    }
    let asked = question.clone();
    ask(&alert, window, move |answer| {
        // Taken out first: one told may ask again, and is asked anew.
        let waiting = UP.with(|up| crate::lock(up).answered(&asked));
        for done in waiting {
            done(answer == FIRST);
        }
    });
}

/// A folder the person picks, on `window` (or on its own, when it is away), folders only; `done`
/// takes it, or none when they cancel.
pub(in super::super) fn pick_folder(
    window: Option<&tao::window::Window>,
    done: impl FnOnce(Option<std::path::PathBuf>) + 'static,
) {
    /// NSModalResponseOK: the person chose.
    const CHOSE: isize = 1;
    // SAFETY: NSOpenPanel's class method answers a new panel, retained by the class.
    let panel: Option<Retained<AnyObject>> = unsafe { msg_send![class!(NSOpenPanel), openPanel] };
    let Some(panel) = panel else {
        return done(None);
    };
    // SAFETY: an open panel takes BOOLs: folders yes, files no, one at a time.
    let _: () = unsafe { msg_send![&*panel, setCanChooseDirectories: Bool::YES] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*panel, setCanChooseFiles: Bool::NO] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*panel, setAllowsMultipleSelection: Bool::NO] };
    // SAFETY: as above: a folder may be made in the panel.
    let _: () = unsafe { msg_send![&*panel, setCanCreateDirectories: Bool::YES] };
    let open = text("Open");
    // SAFETY: an open panel takes its button's title, an NSString.
    let _: () = unsafe { msg_send![&*panel, setPrompt: &*open] };
    // The panel is read once it is answered; the handler lets it go then.
    let chosen = panel.clone();
    ask(&panel, window, move |answer| {
        done((answer == CHOSE).then(|| chosen_path(&chosen)).flatten());
    });
}

/// The folder an answered open panel holds, if any.
fn chosen_path(panel: &AnyObject) -> Option<std::path::PathBuf> {
    // SAFETY: an open panel answers its chosen URL, or nil.
    let url: Option<Retained<AnyObject>> = unsafe { msg_send![panel, URL] };
    // SAFETY: an NSURL answers its path, an NSString, or nil.
    let path: Option<Retained<AnyObject>> = unsafe { msg_send![&*url?, path] };
    let path = path?;
    // SAFETY: an NSString answers its UTF-8 bytes, alive while the string is.
    let bytes: *const std::ffi::c_char = unsafe { msg_send![&*path, UTF8String] };
    if bytes.is_null() {
        return None;
    }
    // SAFETY: a NUL-terminated string the NSString holds, read at once.
    let path = unsafe { CStr::from_ptr(bytes) };
    Some(std::path::PathBuf::from(
        path.to_string_lossy().into_owned(),
    ))
}

#[cfg(test)]
#[path = "sheet_tests.rs"]
mod tests;
