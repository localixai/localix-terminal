//! A choice put to the person as the system's own menu (macOS), where they clicked: what the
//! strip's buttons offer ("+": a page or a terminal). It looks and behaves as every menu of the
//! system does, covers nothing but itself, and answers the item picked, or none.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicIsize, Ordering};

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use wry::{WebView, WebViewExtMacOS};

use super::capture::CGPoint;
use super::choice::Entry;

/// The item the person picked in the menu shown now: its tag, -1 for none.
static PICKED: AtomicIsize = AtomicIsize::new(-1);

extern "C" fn pick(_this: &AnyObject, _sel: Sel, sender: *mut AnyObject) {
    // SAFETY: the sender is the NSMenuItem picked, alive while its action runs.
    let Some(item) = (unsafe { sender.as_ref() }) else {
        return;
    };
    // SAFETY: an NSMenuItem answers its tag, an NSInteger.
    let tag: isize = unsafe { msg_send![item, tag] };
    PICKED.store(tag, Ordering::SeqCst);
}

/// What the items call: one object for the process.
fn picker() -> Option<&'static AnyObject> {
    static TARGET: OnceLock<Option<usize>> = OnceLock::new();
    let at = (*TARGET.get_or_init(|| {
        let mut class = ClassBuilder::new(c"LlxMenuPick", AnyClass::get(c"NSObject")?)?;
        // SAFETY: an action's signature: the receiver, the selector, the sender.
        unsafe { class.add_method(sel!(pick:), pick as extern "C" fn(_, _, _)) };
        let class = class.register();
        // SAFETY: `new` on an NSObject subclass answers a retained instance.
        let object: Retained<AnyObject> = unsafe { msg_send![class, new] };
        // Kept for the process: a menu item holds its target weakly.
        Some(Retained::into_raw(object) as usize)
    }))?;
    // SAFETY: the object was made above and never released.
    unsafe { (at as *const AnyObject).as_ref() }
}

/// `svg` as a menu item's picture: 16 points, a template the menu tints as the item's text.
fn picture(svg: &str) -> Option<Retained<AnyObject>> {
    /// A menu item's picture, as the system draws its own.
    const SIDE: f64 = 16.0;
    // SAFETY: NSData copies the bytes it is given, of the length given.
    let data: Retained<AnyObject> = unsafe {
        msg_send![class!(NSData), dataWithBytes: svg.as_ptr().cast::<std::ffi::c_void>(), length: svg.len()]
    };
    // SAFETY: `alloc` answers an NSImage to initialise.
    let blank: Allocated<AnyObject> = unsafe { msg_send![class!(NSImage), alloc] };
    // SAFETY: `initWithData:` answers the image, or nil when the data is no picture AppKit reads
    // (SVG it reads since macOS 14).
    let image: Option<Retained<AnyObject>> = unsafe { msg_send![blank, initWithData: &*data] };
    let image = image?;
    let size = super::capture::CGSize {
        width: SIDE,
        height: SIDE,
    };
    // SAFETY: an NSImage takes the size it is drawn at, and a BOOL: whether it is a template.
    let _: () = unsafe { msg_send![&*image, setSize: size] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*image, setTemplate: Bool::YES] };
    Some(image)
}

/// NSEvent's modifier flags.
pub(super) const SHIFT: usize = 1 << 17;
pub(super) const CONTROL: usize = 1 << 18;
pub(super) const OPTION: usize = 1 << 19;
pub(super) const COMMAND: usize = 1 << 20;

/// The modifier flags of `chord`, `shift` the shift its key takes as typed.
pub(super) fn modifiers(chord: &super::shortcuts::Chord, shift: bool) -> usize {
    [
        (chord.cmd, COMMAND),
        (chord.ctrl, CONTROL),
        (chord.alt, OPTION),
        (shift, SHIFT),
    ]
    .iter()
    .filter(|(held, _)| *held)
    .fold(0, |mask, (_, bit)| mask | bit)
}

/// Show `entries` as a menu at `at` of `view` (its points, from its top left) and wait for the
/// person: the place of the item picked among the menu's items, or none when they dismissed it.
pub(super) fn show(view: &WebView, at: (f64, f64), entries: &[Entry]) -> Option<usize> {
    let target = picker()?;
    let menu = filled(entries, target, &mut 0)?;
    PICKED.store(-1, Ordering::SeqCst);
    let view = view.webview();
    let point = CGPoint { x: at.0, y: at.1 };
    // The menu runs until the person picks or dismisses it; the window keeps drawing meanwhile.
    // A WKWebView's coordinates run downwards, as the page's do.
    // SAFETY: a menu pops up over a view, at a point of it, with no item placed under it.
    let _: Bool = unsafe {
        msg_send![&*menu, popUpMenuPositioningItem: std::ptr::null::<AnyObject>(), atLocation: point, inView: &*view]
    };
    usize::try_from(PICKED.load(Ordering::SeqCst)).ok()
}

/// `item` shows `chord` at its right, as the system shows an item's keys.
fn shortcut(item: &AnyObject, chord: &super::shortcuts::Chord) {
    let (key, shift) = chord.menu_key();
    let Some(key) = super::appkit::string(&key) else {
        return;
    };
    // SAFETY: an NSMenuItem takes its key equivalent (an NSString) and keeps it.
    let _: () = unsafe { msg_send![item, setKeyEquivalent: &*key] };
    // SAFETY: an NSMenuItem takes its key equivalent's modifier flags (an NSUInteger).
    let _: () = unsafe { msg_send![item, setKeyEquivalentModifierMask: modifiers(chord, shift)] };
}

/// A menu of `entries`, its items tagged from `next` on.
fn filled(entries: &[Entry], target: &AnyObject, next: &mut isize) -> Option<Retained<AnyObject>> {
    // SAFETY: `new` on NSMenu answers a retained, empty menu.
    let menu: Retained<AnyObject> = unsafe { msg_send![class!(NSMenu), new] };
    // SAFETY: a menu takes a BOOL: its items are enabled as they are set, not by a responder.
    let _: () = unsafe { msg_send![&*menu, setAutoenablesItems: Bool::NO] };
    let none = super::appkit::string("")?;
    for entry in entries {
        match entry {
            Entry::Line { .. } => {
                // SAFETY: NSMenuItem answers the shared separator item; a menu adds an item.
                let line: *mut AnyObject = unsafe { msg_send![class!(NSMenuItem), separatorItem] };
                // SAFETY: as above; the separator item is alive, autoreleased.
                if let Some(line) = unsafe { line.as_ref() } {
                    // SAFETY: a menu adds an NSMenuItem and keeps it.
                    let _: () = unsafe { msg_send![&*menu, addItem: line] };
                }
            }
            Entry::Item {
                label,
                marked,
                off,
                icon,
                key,
                ..
            } => {
                let Some(title) = super::appkit::string(label) else {
                    *next += 1;
                    continue;
                };
                // SAFETY: a menu adds an item of a title, an action and a key, and answers it.
                let item: *mut AnyObject = unsafe {
                    msg_send![&*menu, addItemWithTitle: &*title, action: sel!(pick:), keyEquivalent: &*none]
                };
                // SAFETY: the menu keeps the item it answered.
                if let Some(item) = unsafe { item.as_ref() } {
                    // SAFETY: an NSMenuItem takes a target.
                    let _: () = unsafe { msg_send![item, setTarget: target] };
                    // SAFETY: an NSMenuItem takes an NSInteger tag.
                    let _: () = unsafe { msg_send![item, setTag: *next] };
                    // SAFETY: an NSMenuItem takes a BOOL: whether it can be picked.
                    let _: () = unsafe { msg_send![item, setEnabled: Bool::new(!off)] };
                    if let Some(chord) = key.as_deref().and_then(super::shortcuts::Chord::parse) {
                        shortcut(item, &chord);
                    }
                    if let Some(picture) = icon.and_then(picture) {
                        // SAFETY: an NSMenuItem takes an NSImage and keeps it.
                        let _: () = unsafe { msg_send![item, setImage: &*picture] };
                    }
                    if *marked {
                        /// NSControlStateValueOn: the item's check mark.
                        const ON: isize = 1;
                        // SAFETY: an NSMenuItem takes an NSControlStateValue (an NSInteger).
                        let _: () = unsafe { msg_send![item, setState: ON] };
                    }
                }
                *next += 1;
            }
            Entry::Sub { label, items } => {
                let (Some(title), Some(sub)) =
                    (super::appkit::string(label), filled(items, target, next))
                else {
                    continue;
                };
                // SAFETY: as for an item, with no action: it opens its submenu.
                let item: *mut AnyObject = unsafe {
                    msg_send![&*menu, addItemWithTitle: &*title, action: None::<Sel>, keyEquivalent: &*none]
                };
                // SAFETY: the menu keeps the item it answered; an NSMenuItem takes a submenu.
                if let Some(item) = unsafe { item.as_ref() } {
                    // SAFETY: an NSMenuItem takes a submenu and keeps it.
                    let _: () = unsafe { msg_send![item, setSubmenu: &*sub] };
                }
            }
        }
    }
    Some(menu)
}
