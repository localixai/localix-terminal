//! The terminal's view to VoiceOver (NSAccessibility): a text area whose text is the screen's
//! lines as the view drew them, one line of text a row, the cursor's line the one with the
//! insertion point, read by range and by line; and, while VoiceOver runs, the new output read
//! out as it comes (`spoken.rs`), as the terminal page's screen reader mode read it: at once,
//! interrupting (its live region was assertive), the echo of the keys typed left out; and the
//! view's end, told as it closes.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, class, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSAttributedString, NSRange, NSString};

use super::view::TermView;

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    fn NSAccessibilityPostNotification(element: *const AnyObject, name: *const NSString);
    fn NSAccessibilityPostNotificationWithUserInfo(
        element: *const AnyObject,
        name: *const NSString,
        info: *const AnyObject,
    );
}

/// NSAccessibilityPriorityHigh: read at once, over what is being read, as an assertive region.
const PRIORITY_HIGH: isize = 90;

/// The screen's text as VoiceOver reads it: its lines, and where each begins (in UTF-16 units,
/// as an NSString counts).
pub(super) struct Text {
    pub(super) whole: Retained<NSString>,
    starts: Vec<usize>,
    lengths: Vec<usize>,
}

/// The rows the view drew last, top to bottom, each with its trailing blanks cut.
pub(super) fn lines(view: &TermView) -> Vec<String> {
    view.state()
        .map(|state| state.lines.of(&state.frame))
        .unwrap_or_default()
}

fn text(view: &TermView) -> Text {
    let lines = lines(view);
    let (mut starts, mut lengths, mut at) = (vec![], vec![], 0);
    for line in &lines {
        let length = line.encode_utf16().count();
        starts.push(at);
        lengths.push(length);
        at += length + 1;
    }
    Text {
        whole: NSString::from_str(&lines.join("\n")),
        starts,
        lengths,
    }
}

impl Text {
    fn length(&self) -> usize {
        self.whole.length()
    }

    /// The line `index` is in.
    fn line_of(&self, index: usize) -> usize {
        self.starts
            .partition_point(|&start| start <= index)
            .saturating_sub(1)
    }

    /// Line `line`, with the newline after it but for the last.
    fn range_of(&self, line: usize) -> NSRange {
        let (Some(&start), Some(&length)) = (self.starts.get(line), self.lengths.get(line)) else {
            return NSRange::new(self.length(), 0);
        };
        let newline = usize::from(line + 1 < self.starts.len());
        NSRange::new(start, length + newline)
    }

    /// `range` held within the text.
    fn clamped(&self, range: NSRange) -> NSRange {
        let start = range.location.min(self.length());
        NSRange::new(start, range.length.min(self.length() - start))
    }
}

pub(super) fn role() -> Retained<NSString> {
    NSString::from_str("AXTextArea")
}

pub(super) fn label() -> Retained<NSString> {
    NSString::from_str("Terminal")
}

pub(super) fn value(view: &TermView) -> Retained<NSString> {
    text(view).whole
}

pub(super) fn characters(view: &TermView) -> isize {
    isize::try_from(text(view).length()).unwrap_or(isize::MAX)
}

pub(super) fn visible(view: &TermView) -> NSRange {
    NSRange::new(0, text(view).length())
}

/// Where the cursor is, as an insertion point: its row's line, its column within it.
pub(super) fn selected_range(view: &TermView) -> NSRange {
    let at = view.state().and_then(|state| state.frame.cursor.at);
    let text = text(view);
    let Some((column, row)) = at else {
        return NSRange::new(text.length(), 0);
    };
    let line = text.range_of(usize::from(row));
    let length = text.lengths.get(usize::from(row)).copied().unwrap_or(0);
    NSRange::new(line.location + usize::from(column).min(length), 0)
}

/// What the person selected, as Copy would take it.
pub(super) fn selected_text(view: &TermView) -> Retained<NSString> {
    let copied = super::events::surface(view).and_then(|surface| surface.copy());
    NSString::from_str(&copied.unwrap_or_default())
}

pub(super) fn insertion_line(view: &TermView) -> isize {
    let at = view.state().and_then(|state| state.frame.cursor.at);
    at.map_or(0, |(_, row)| {
        isize::from(i16::try_from(row).unwrap_or(i16::MAX))
    })
}

pub(super) fn line_for(view: &TermView, index: isize) -> isize {
    let line = text(view).line_of(index.max(0).unsigned_abs());
    isize::try_from(line).unwrap_or(isize::MAX)
}

pub(super) fn range_for(view: &TermView, line: isize) -> NSRange {
    text(view).range_of(line.max(0).unsigned_abs())
}

pub(super) fn string_for(view: &TermView, range: NSRange) -> Retained<NSString> {
    let text = text(view);
    let range = text.clamped(range);
    // SAFETY: an NSString answers the part of it a range within it covers.
    unsafe { msg_send![&*text.whole, substringWithRange: range] }
}

pub(super) fn attributed_for(view: &TermView, range: NSRange) -> Retained<NSAttributedString> {
    let string = string_for(view, range);
    NSAttributedString::initWithString(NSAttributedString::alloc(), &string)
}

/// Where on the screen the lines `range` begins in are: the whole width of its row.
pub(super) fn frame_for(view: &TermView, range: NSRange) -> CGRect {
    let line = text(view).line_of(range.location);
    let (height, top) = view.state().map_or((0.0, 0.0), |state| {
        let height = state
            .painter
            .as_ref()
            .map_or(0.0, |painter| painter.cell().1);
        (
            height,
            f64::from(super::layout::gutter(state.frame.alternate).top),
        )
    });
    let bounds = super::ns::bounds(view);
    #[expect(
        clippy::cast_precision_loss,
        reason = "a row of a screen, far below 2^52"
    )]
    let y = top + line as f64 * height;
    let inside = CGRect::new(CGPoint::new(0.0, y), CGSize::new(bounds.size.width, height));
    let none = std::ptr::null::<AnyObject>();
    // SAFETY: a view converts a rect of its own into its window's (nil).
    let in_window: CGRect = unsafe { msg_send![view, convertRect: inside, toView: none] };
    super::ns::window(view).map_or(in_window, |window| {
        // SAFETY: a window converts a rect of its own into the screen's.
        unsafe { msg_send![&*window, convertRectToScreen: in_window] }
    })
}

/// Whether the view speaks to a screen reader: VoiceOver runs, or a check says it does.
fn reading(view: &TermView) -> bool {
    let told = view.state().and_then(|state| state.reader);
    told.unwrap_or_else(crate::browser::frame::screen_reader)
}

/// The view drew a new frame: VoiceOver is told its text changed, and what is new in it is read
/// out. Nothing while no screen reader runs, and the next one starts from a first look.
pub(super) fn drew(view: &TermView) {
    if !reading(view) {
        if let Some(mut state) = view.state() {
            state.spoken.forget();
        }
        return;
    }
    let lines = lines(view);
    let said = view.state().and_then(|mut state| {
        let said = state.spoken.heard(&lines)?;
        said.clone_into(&mut state.said);
        Some(said)
    });
    let element: *const AnyObject = std::ptr::from_ref(view).cast();
    let changed = NSString::from_str("AXValueChanged");
    // SAFETY: the system's own call, the view and the notification's name alive through it.
    unsafe { NSAccessibilityPostNotification(element, &raw const *changed) };
    if let Some(said) = said {
        announce(element, &said);
    }
}

/// The view is gone: VoiceOver forgets it at once.
pub(super) fn gone(view: &TermView) {
    let element: *const AnyObject = std::ptr::from_ref(view).cast();
    let destroyed = NSString::from_str("AXUIElementDestroyed");
    // SAFETY: the system's own call, the view and the notification's name alive through it.
    unsafe { NSAccessibilityPostNotification(element, &raw const *destroyed) };
}

/// `said` read out at once by VoiceOver, on behalf of `element`.
fn announce(element: *const AnyObject, said: &str) {
    let (text, priority) = (NSString::from_str(said), PRIORITY_HIGH);
    // SAFETY: NSNumber's class method takes an NSInteger and answers a number.
    let priority: Retained<AnyObject> =
        unsafe { msg_send![class!(NSNumber), numberWithInteger: priority] };
    let (text_key, priority_key) = (
        NSString::from_str("AXAnnouncementKey"),
        NSString::from_str("AXPriorityKey"),
    );
    // SAFETY: NSMutableDictionary's class method answers a new, empty dictionary.
    let info: Retained<AnyObject> = unsafe { msg_send![class!(NSMutableDictionary), dictionary] };
    // SAFETY: a mutable dictionary takes an object for a key, both alive and retained by it.
    let _: () = unsafe { msg_send![&*info, setObject: &*text, forKey: &*text_key] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*info, setObject: &*priority, forKey: &*priority_key] };
    let name = NSString::from_str("AXAnnouncementRequested");
    // SAFETY: the system's own call, every argument alive through it.
    unsafe {
        NSAccessibilityPostNotificationWithUserInfo(element, &raw const *name, &raw const *info)
    };
}
