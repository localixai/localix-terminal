//! AppKit's few common messages, once (macOS): a string, the app, the person's question.

use std::ffi::CStr;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};

/// `text` as an NSString; none when it holds a NUL.
pub(super) fn string(text: &str) -> Option<Retained<AnyObject>> {
    let text = std::ffi::CString::new(text).ok()?;
    literal(&text)
}

/// `text` as an NSString, any NUL in it left out.
pub(super) fn text(text: &str) -> Retained<AnyObject> {
    string(&text.replace('\0', "")).unwrap_or_else(|| {
        // SAFETY: `new` on NSString answers an empty string, retained.
        unsafe { msg_send![class!(NSString), new] }
    })
}

/// A C string as an NSString (a constant's name, a key).
pub(super) fn literal(text: &CStr) -> Option<Retained<AnyObject>> {
    // SAFETY: a NUL-terminated UTF-8 string, alive through the call; it answers a new string.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()] }
}

/// The app (NSApplication).
pub(super) fn app() -> Retained<AnyObject> {
    // SAFETY: NSApplication's class method answers the app.
    unsafe { msg_send![class!(NSApplication), sharedApplication] }
}

/// The app made the one the person uses: only ever for what they did themselves (a click on
/// the mini, the Dock, quitting), never for what an agent does.
pub(super) fn activate() {
    // SAFETY: the app takes a BOOL: whether to take over from the app active now.
    let _: () = unsafe { msg_send![&*app(), activateIgnoringOtherApps: Bool::YES] };
}

/// The person's answer to `question`, asked as the system asks it (an alert, in front of every
/// app): whether they chose `yes` over Cancel.
pub(super) fn confirm(question: &super::Confirm) -> bool {
    /// NSAlertFirstButtonReturn: the first button added, `yes`.
    const FIRST: isize = 1000;
    activate();
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
    // SAFETY: on the main thread, as the event loop is: the alert runs until it is answered.
    let answer: isize = unsafe { msg_send![&*alert, runModal] };
    answer == FIRST
}

/// The shared workspace (NSWorkspace): what opens files and addresses as the system does.
fn workspace() -> Retained<AnyObject> {
    // SAFETY: NSWorkspace's class method answers the shared workspace.
    unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] }
}

/// `address` opened by the app the person chose for it (their default browser, for a web
/// page); whether the system took it.
pub(super) fn open_outside(address: &str) -> bool {
    let Some(text) = string(address) else {
        return false;
    };
    // SAFETY: NSURL's class method takes an NSString and answers an NSURL, or nil when the
    // text is no address.
    let url: Option<Retained<AnyObject>> =
        unsafe { msg_send![class!(NSURL), URLWithString: &*text] };
    let Some(url) = url else {
        return false;
    };
    // SAFETY: the workspace opens an NSURL, answering a BOOL.
    let opened: Bool = unsafe { msg_send![&*workspace(), openURL: &*url] };
    opened.as_bool()
}

/// The file or folder at `path` shown, selected, in the Finder; whether it was.
pub(super) fn reveal(path: &std::path::Path) -> bool {
    let (path, root) = (text(&path.to_string_lossy()), text(""));
    // SAFETY: the workspace takes two NSStrings (the path, and "" for a new Finder window) and
    // answers a BOOL.
    let shown: Bool =
        unsafe { msg_send![&*workspace(), selectFile: &*path, inFileViewerRootedAtPath: &*root] };
    shown.as_bool()
}

/// A folder the person picks, as the system asks (an open panel, folders only); none when they
/// cancel.
pub(super) fn pick_folder() -> Option<std::path::PathBuf> {
    /// NSModalResponseOK: the person chose.
    const CHOSE: isize = 1;
    activate();
    // SAFETY: NSOpenPanel's class method answers a new panel, retained by the class.
    let panel: Option<Retained<AnyObject>> = unsafe { msg_send![class!(NSOpenPanel), openPanel] };
    let panel = panel?;
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
    // SAFETY: on the main thread, as the event loop is: the panel runs until it is answered.
    let answer: isize = unsafe { msg_send![&*panel, runModal] };
    if answer != CHOSE {
        return None;
    }
    // SAFETY: an open panel answers its chosen URL, or nil.
    let url: Option<Retained<AnyObject>> = unsafe { msg_send![&*panel, URL] };
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
