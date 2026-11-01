//! The system's pasteboard (macOS), for what a terminal's menu copies and pastes: plain text.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

/// NSPasteboardTypeString.
const PLAIN: &std::ffi::CStr = c"public.utf8-plain-text";

fn general() -> Option<Retained<AnyObject>> {
    // SAFETY: NSPasteboard answers the general pasteboard.
    unsafe { msg_send![class!(NSPasteboard), generalPasteboard] }
}

fn string(text: &std::ffi::CStr) -> Option<Retained<AnyObject>> {
    // SAFETY: a NUL-terminated UTF-8 string, alive through the call; it answers a new string.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()] }
}

/// Put `text` on the pasteboard, in place of what was there.
pub(super) fn copy(text: &str) {
    let (Some(board), Ok(text)) = (general(), std::ffi::CString::new(text)) else {
        return;
    };
    let (Some(text), Some(plain)) = (string(&text), string(PLAIN)) else {
        return;
    };
    // SAFETY: a pasteboard clears its contents and answers a change count.
    let _: isize = unsafe { msg_send![&*board, clearContents] };
    // SAFETY: a pasteboard takes a string of a type, and answers whether it took it.
    let _: objc2::runtime::Bool =
        unsafe { msg_send![&*board, setString: &*text, forType: &*plain] };
}

/// The pasteboard's text, if it holds any.
pub(super) fn pasted() -> Option<String> {
    let board = general()?;
    let plain = string(PLAIN)?;
    // SAFETY: a pasteboard answers its string of a type, or nil.
    let text: Option<Retained<AnyObject>> = unsafe { msg_send![&*board, stringForType: &*plain] };
    let text = text?;
    // SAFETY: an NSString answers its UTF-8, alive as long as the string, or null.
    let utf8: *const std::ffi::c_char = unsafe { msg_send![&*text, UTF8String] };
    if utf8.is_null() {
        return None;
    }
    // SAFETY: not null (checked above); NUL-terminated, and `text` outlives this read.
    let read = unsafe { std::ffi::CStr::from_ptr(utf8) };
    Some(read.to_string_lossy().into_owned())
}
