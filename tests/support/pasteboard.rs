//! The person's pasteboard around a check that could write to it (macOS): every item, of every
//! type, kept as it was and put back when the check ends, a failed one too, but only if what it
//! holds then is the check's own text: what the person copied meanwhile stays. And what it holds
//! as text.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

/// NSPasteboardTypeString.
const PLAIN: &std::ffi::CStr = c"public.utf8-plain-text";

fn general() -> Retained<AnyObject> {
    // SAFETY: NSPasteboard answers the general pasteboard.
    unsafe { msg_send![class!(NSPasteboard), generalPasteboard] }
}

/// The items of an NSArray.
fn each(array: &AnyObject) -> Vec<Retained<AnyObject>> {
    // SAFETY: an NSArray answers its count, an NSUInteger.
    let count: usize = unsafe { msg_send![array, count] };
    // SAFETY: an NSArray answers its object at an index below its count.
    (0..count)
        .map(|at| unsafe { msg_send![array, objectAtIndex: at] })
        .collect()
}

/// An NSString of `text`.
fn string(text: &std::ffi::CStr) -> Retained<AnyObject> {
    // SAFETY: NSString makes a string of a NUL-terminated UTF-8 C string.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()] }
}

/// What the pasteboard held, each item its types' data, and the check's own text (`ours`); put
/// back as it drops, when the pasteboard holds `ours`.
pub(crate) struct Saved {
    items: Vec<Vec<(Retained<AnyObject>, Retained<AnyObject>)>>,
    ours: String,
}

impl Saved {
    pub(crate) fn now(ours: &str) -> Self {
        // SAFETY: a pasteboard answers its items, an NSArray of NSPasteboardItem, or nil.
        let items: Option<Retained<AnyObject>> = unsafe { msg_send![&*general(), pasteboardItems] };
        let items = items.map(|items| each(&items)).unwrap_or_default();
        let items = items
            .iter()
            .map(|item| {
                // SAFETY: a pasteboard item answers its types, an NSArray of NSString.
                let types: Retained<AnyObject> = unsafe { msg_send![&**item, types] };
                (each(&types).into_iter())
                    .filter_map(|kind| {
                        // SAFETY: an item answers its data of one of its types, or nil.
                        let data: Option<Retained<AnyObject>> =
                            unsafe { msg_send![&**item, dataForType: &*kind] };
                        Some((kind, data?))
                    })
                    .collect()
            })
            .collect();
        Self {
            items,
            ours: ours.to_owned(),
        }
    }
}

impl Drop for Saved {
    fn drop(&mut self) {
        if pasted() != self.ours {
            return;
        }
        let board = general();
        // SAFETY: a pasteboard clears its contents and answers a change count.
        let _: isize = unsafe { msg_send![&*board, clearContents] };
        // SAFETY: NSMutableArray makes an empty array.
        let items: Retained<AnyObject> = unsafe { msg_send![class!(NSMutableArray), array] };
        for kept in &self.items {
            // SAFETY: NSPasteboardItem makes an empty item.
            let item: Retained<AnyObject> = unsafe { msg_send![class!(NSPasteboardItem), new] };
            for (kind, data) in kept {
                // SAFETY: an item takes data for a type, and answers whether it took it.
                let _: objc2::runtime::Bool =
                    unsafe { msg_send![&*item, setData: &**data, forType: &**kind] };
            }
            // SAFETY: a mutable array takes an object.
            let _: () = unsafe { msg_send![&*items, addObject: &*item] };
        }
        // SAFETY: a pasteboard takes an array of items to write, and answers whether it did.
        let _: objc2::runtime::Bool = unsafe { msg_send![&*board, writeObjects: &*items] };
    }
}

/// The pasteboard's text now, empty when it holds none.
pub(crate) fn pasted() -> String {
    let plain = string(PLAIN);
    // SAFETY: a pasteboard answers its string of a type, or nil.
    let text: Option<Retained<AnyObject>> =
        unsafe { msg_send![&*general(), stringForType: &*plain] };
    let Some(text) = text else {
        return String::new();
    };
    // SAFETY: an NSString answers its UTF-8, alive as long as the string.
    let utf8: *const std::ffi::c_char = unsafe { msg_send![&*text, UTF8String] };
    // SAFETY: not null (an NSString always has one), NUL-terminated, and `text` outlives it.
    unsafe { std::ffi::CStr::from_ptr(utf8) }
        .to_string_lossy()
        .into_owned()
}

/// `text` put on the pasteboard alone, as the person copying it would.
pub(crate) fn put(text: &str) {
    let text = std::ffi::CString::new(text).unwrap();
    let (board, text, plain) = (general(), string(&text), string(PLAIN));
    // SAFETY: a pasteboard clears its contents and answers a change count.
    let _: isize = unsafe { msg_send![&*board, clearContents] };
    // SAFETY: a pasteboard takes a string of a type, and answers whether it took it.
    let _: objc2::runtime::Bool =
        unsafe { msg_send![&*board, setString: &*text, forType: &*plain] };
}
