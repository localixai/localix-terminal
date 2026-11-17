//! For checks, the rest of what a check tells a terminal's view as the system would: the
//! pointer in the scrollbar's column, the wheel turned, the Edit menu's Copy, Paste and Select
//! All, the keys given to the view or taken from it as in the key window, the system's display options (more
//! contrast, less transparency) as the person would set them, and VoiceOver's questions, asked
//! through the same methods VoiceOver calls; how many terminal views a window holds in its
//! hierarchy, where one lies in it, and a person's left button pressed, dragged and let go at a
//! place of the window through the app's queue of events. Nothing else calls it.

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{ClassType, class, msg_send, sel};
use objc2_core_foundation::CGPoint;
use objc2_foundation::{NSRange, NSString};

use super::super::{Edit, Pointing, Reader, TermAnswer};
use super::overlay::SCROLLBAR_WIDTH;
use super::probe::{kind, point_event};
use super::view::TermView;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateScrollWheelEvent2(
        source: *const c_void,
        units: u32,
        count: u32,
        first: i32,
        second: i32,
        third: i32,
    ) -> *mut c_void;
    fn CFRelease(object: *const c_void);
}

/// kCGScrollEventUnitLine: the wheel turned by rows, as a mouse's wheel turns.
const LINES: u32 = 1;

fn text(string: Option<Retained<NSString>>) -> String {
    string.map(|string| string.to_string()).unwrap_or_default()
}

impl TermView {
    /// The pointer at the scrollbar's column, `percent` of the way down the view; pressed, it
    /// first came into the view, as the system says when it does.
    pub(super) fn bar_probe(&self, pointing: Pointing, percent: u8) -> Option<TermAnswer> {
        let bounds = super::ns::bounds(self);
        let at = CGPoint::new(
            bounds.size.width - SCROLLBAR_WIDTH / 2.0,
            bounds.size.height * f64::from(percent.min(100)) / 100.0,
        );
        let event = point_event(self, kind(pointing), at, 0)?;
        if pointing == Pointing::Down {
            // SAFETY: a responder takes the pointer coming into its tracking area.
            let _: () = unsafe { msg_send![self, mouseEntered: &*event] };
        }
        self.pointed(pointing, &event);
        Some(TermAnswer::Done)
    }

    /// The wheel turned `rows` rows (up when more than 0), over the view.
    pub(super) fn wheel_probe(&self, rows: i32) -> Option<TermAnswer> {
        // SAFETY: the system makes a scroll event of no source, retained, or null.
        let made =
            unsafe { CGEventCreateScrollWheelEvent2(std::ptr::null(), LINES, 1, rows, 0, 0) };
        if made.is_null() {
            return None;
        }
        // SAFETY: an NSEvent is made of a live CGEvent, autoreleased, or nil.
        let event: Option<Retained<AnyObject>> =
            unsafe { msg_send![class!(NSEvent), eventWithCGEvent: made] };
        // SAFETY: the event made above, released once, here (the NSEvent holds its own).
        unsafe { CFRelease(made) };
        // SAFETY: a responder takes a scroll event as the system hands it over.
        let _: () = unsafe { msg_send![self, scrollWheel: &*event?] };
        Some(TermAnswer::Done)
    }

    /// `edit` sent as the Edit menu sends it, with no sender.
    pub(super) fn edit_probe(&self, edit: Edit) -> TermAnswer {
        let none = std::ptr::null::<AnyObject>();
        let selector = match edit {
            Edit::Copy => sel!(copy:),
            Edit::Paste => sel!(paste:),
            Edit::SelectAll => sel!(selectAll:),
        };
        // SAFETY: the view's own action methods, each taking a sender, here nil.
        let _: *mut AnyObject =
            unsafe { msg_send![self, performSelector: selector, withObject: none] };
        TermAnswer::Done
    }

    /// The keys given to the view, or its window's taken from it (the window keeps them), in a
    /// window that is the key one (a test run is never the app in front).
    pub(super) fn focus_probe(&self, on: bool) -> TermAnswer {
        if on {
            self.focus();
        } else if let Some(window) = super::ns::window(self) {
            let none = std::ptr::null::<AnyObject>();
            // SAFETY: a window makes a responder its first, here none but itself.
            let _: bool = unsafe { msg_send![&*window, makeFirstResponder: none] };
        }
        super::events::focused_in(self, on, true);
        TermAnswer::Done
    }

    /// The display options a check sets, and the look they make.
    pub(super) fn display_probe(&self, more: bool, solid: bool) -> TermAnswer {
        if let Some(mut state) = self.state() {
            state.display = Some((more, solid));
        }
        self.restyle();
        TermAnswer::Done
    }

    /// A screen reader running from now on, and what it is told of the view.
    pub(super) fn reader_probe(&self) -> TermAnswer {
        if let Some(mut state) = self.state() {
            state.reader = Some(true);
        }
        // SAFETY: each of the view's own accessibility methods, as VoiceOver calls them.
        let role: Option<Retained<NSString>> = unsafe { msg_send![self, accessibilityRole] };
        // SAFETY: as above.
        let label: Option<Retained<NSString>> = unsafe { msg_send![self, accessibilityLabel] };
        // SAFETY: as above.
        let value: Option<Retained<NSString>> = unsafe { msg_send![self, accessibilityValue] };
        // SAFETY: as above.
        let characters: isize = unsafe { msg_send![self, accessibilityNumberOfCharacters] };
        // SAFETY: as above.
        let line: isize = unsafe { msg_send![self, accessibilityInsertionPointLineNumber] };
        // SAFETY: as above.
        let range: NSRange = unsafe { msg_send![self, accessibilityRangeForLine: line] };
        // SAFETY: as above.
        let line_text: Option<Retained<NSString>> =
            unsafe { msg_send![self, accessibilityStringForRange: range] };
        TermAnswer::Reader(Reader {
            role: text(role),
            label: text(label),
            value: text(value),
            characters: characters.max(0).unsigned_abs(),
            line: line.max(0).unsigned_abs(),
            line_text: text(line_text).trim_end().to_string(),
            said: self
                .state()
                .map(|state| state.said.clone())
                .unwrap_or_default(),
        })
    }

    /// How many terminal views are in `native`'s content view.
    pub(crate) fn placed_in(native: &tao::window::Window) -> usize {
        use tao::platform::macos::WindowExtMacOS;
        let content: *mut AnyObject = native.ns_view().cast();
        // SAFETY: the window's content view, alive with the window.
        let Some(content) = (unsafe { content.as_ref() }) else {
            return 0;
        };
        // SAFETY: an NSView answers its subviews, an NSArray.
        let subviews: Retained<AnyObject> = unsafe { msg_send![content, subviews] };
        // SAFETY: an NSArray answers how many it holds, an NSUInteger.
        let count: usize = unsafe { msg_send![&*subviews, count] };
        let class = Self::class();
        (0..count)
            .filter(|&at| {
                // SAFETY: an NSArray answers the object at an index below its count.
                let view: Retained<AnyObject> = unsafe { msg_send![&*subviews, objectAtIndex: at] };
                // SAFETY: an object answers whether it is of a class, a BOOL.
                let kind: bool = unsafe { msg_send![&*view, isKindOfClass: class] };
                kind
            })
            .count()
    }

    /// For checks: where the view lies in its window, in points from the top left of the
    /// window's content: across, down, wide and tall.
    pub(super) fn window_rect(&self) -> [i32; 4] {
        let Some(content) = super::ns::superview(self) else {
            return [0; 4];
        };
        // SAFETY: an NSView answers its frame, a CGRect in its superview's points.
        let frame: objc2_core_foundation::CGRect = unsafe { msg_send![self, frame] };
        // SAFETY: and whether it counts from the top, a BOOL.
        let flipped: bool = unsafe { msg_send![&*content, isFlipped] };
        let tall = super::ns::bounds(&content).size.height;
        let top = match flipped {
            true => frame.origin.y,
            false => tall - frame.origin.y - frame.size.height,
        };
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a place in a window, in whole points"
        )]
        let whole = |n: f64| n.round() as i32;
        [
            whole(frame.origin.x),
            whole(top),
            whole(frame.size.width),
            whole(frame.size.height),
        ]
    }

    /// For checks: a person's left button at `at`, points from the top left of `native`'s
    /// content, pressed, dragged there or let go: an event made as the system makes one and
    /// put in the app's queue, so the window hands it to whatever view lies under it. Whether
    /// it was put there.
    pub(crate) fn press_in(
        native: &tao::window::Window,
        pointing: Pointing,
        at: (f64, f64),
    ) -> bool {
        use tao::platform::macos::WindowExtMacOS;
        let content: *mut AnyObject = native.ns_view().cast();
        // SAFETY: the window's content view, alive with the window.
        let Some(content) = (unsafe { content.as_ref() }) else {
            return false;
        };
        // SAFETY: an NSView answers whether it counts from the top, a BOOL.
        let flipped: bool = unsafe { msg_send![content, isFlipped] };
        let tall = super::ns::bounds(content).size.height;
        let inside = CGPoint::new(at.0, if flipped { at.1 } else { tall - at.1 });
        // SAFETY: a view converts a point of its own into its window's (nil).
        let point: CGPoint = unsafe {
            msg_send![content, convertPoint: inside, toView: std::ptr::null::<AnyObject>()]
        };
        let window = super::ns::window(content);
        let number: isize = window.as_ref().map_or(0, |window| {
            // SAFETY: a window answers its number, an NSInteger.
            unsafe { msg_send![&**window, windowNumber] }
        });
        // SAFETY: each argument of the type NSEvent's class method declares, in its order; it
        // answers an autoreleased event or nil, which the call retains.
        let event: Option<Retained<AnyObject>> = unsafe {
            msg_send![
                class!(NSEvent),
                mouseEventWithType: kind(pointing),
                location: point,
                modifierFlags: 0_usize,
                timestamp: super::probe::now(),
                windowNumber: number,
                context: std::ptr::null::<AnyObject>(),
                eventNumber: 0_isize,
                clickCount: 1_isize,
                pressure: if pointing == Pointing::Up { 0.0_f32 } else { 1.0_f32 }
            ]
        };
        let Some(event) = event else {
            return false;
        };
        button_held(match pointing {
            Pointing::Down | Pointing::Dragged => true,
            Pointing::Up | Pointing::Moved | Pointing::Flags => false,
        });
        let app = crate::browser::appkit::app();
        // SAFETY: the app takes an event into its queue, at its end.
        let _: () = unsafe { msg_send![&*app, postEvent: &*event, atStart: false] };
        true
    }
}

impl TermView {
    /// For checks: key `code` (typing `chars`) pressed and let go through the app's queue, as a
    /// person's key goes to whatever holds `native`'s keys.
    pub(crate) fn key_in(native: &tao::window::Window, code: u16, chars: &str) -> bool {
        let window = crate::browser::appkit::native(native);
        // SAFETY: a window answers its number, an NSInteger.
        let number: isize = unsafe { msg_send![window, windowNumber] };
        let chars = NSString::from_str(chars);
        let app = crate::browser::appkit::app();
        for kind in [10_usize, 11] {
            // SAFETY: each argument of the type NSEvent's class method declares, in its order;
            // it answers an autoreleased event or nil, which the call retains.
            let event: Option<Retained<AnyObject>> = unsafe {
                msg_send![
                    class!(NSEvent),
                    keyEventWithType: kind,
                    location: CGPoint::new(0.0, 0.0),
                    modifierFlags: 0_usize,
                    timestamp: super::probe::now(),
                    windowNumber: number,
                    context: std::ptr::null::<AnyObject>(),
                    characters: &*chars,
                    charactersIgnoringModifiers: &*chars,
                    isARepeat: objc2::runtime::Bool::NO,
                    keyCode: code
                ]
            };
            let Some(event) = event else {
                return false;
            };
            // SAFETY: the app takes an event into its queue, at its end.
            let _: () = unsafe { msg_send![&*app, postEvent: &*event, atStart: false] };
        }
        true
    }
}

/// The left button held by a check's presses (`TermView::press_in`), and the system's own way of
/// telling which buttons are down, as it was.
static BUTTON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SYSTEMS: std::sync::OnceLock<Option<objc2::runtime::Imp>> = std::sync::OnceLock::new();

/// The buttons down: the hardware's, and the left one while a check's press holds it. A web
/// view reads which buttons a drag holds from the hardware, not from the event; a check's
/// events reach it only so.
unsafe extern "C-unwind" fn pressed_buttons(
    class: *const AnyObject,
    sel: objc2::runtime::Sel,
) -> usize {
    type Buttons = unsafe extern "C-unwind" fn(*const AnyObject, objc2::runtime::Sel) -> usize;
    let hardware = SYSTEMS.get().copied().flatten().map_or(0, |imp| {
        // SAFETY: the system's own method of this selector, which takes its class and selector
        // and answers an NSUInteger.
        let systems: Buttons = unsafe { std::mem::transmute::<objc2::runtime::Imp, Buttons>(imp) };
        // SAFETY: called as the system calls it.
        unsafe { systems(class, sel) }
    });
    hardware | usize::from(BUTTON.load(std::sync::atomic::Ordering::SeqCst))
}

/// The left button held by a check's presses, or let go: NSEvent's `pressedMouseButtons` says
/// so from the first press on, for checks alone (the person's app never presses for them).
fn button_held(on: bool) {
    BUTTON.store(on, std::sync::atomic::Ordering::SeqCst);
    SYSTEMS.get_or_init(|| {
        let method = objc2::runtime::AnyClass::get(c"NSEvent")
            .and_then(|class| class.class_method(sel!(pressedMouseButtons)));
        let ours = pressed_buttons as unsafe extern "C-unwind" fn(_, _) -> usize;
        // SAFETY: a function pointer of the method's own signature, taken as the runtime's IMP.
        let ours: objc2::runtime::Imp = unsafe { std::mem::transmute(ours) };
        // SAFETY: the method answers an NSUInteger from its class and selector, as `ours` does.
        method.map(|method| unsafe { method.set_implementation(ours) })
    });
}
