//! The terminal's view in the window (macOS): an NSView of its own (`TermView`) whose layer is
//! the root of the terminal's layers (`layers.rs`), clear so the window's glass shows through,
//! placed where the tab's pane is. Its size gives the terminal's grid; it takes the keys, the
//! pointer and an input method's text (`events.rs`, `ime.rs`) and tells the terminal through
//! its `Surface`; it draws when the terminal says it changed (`paint.rs`), and while it is out
//! of sight it draws nothing and keeps no picture.

use std::sync::{Arc, Mutex, MutexGuard};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSEvent, NSResponder, NSTextInputClient, NSView};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSArray, NSAttributedString, NSAttributedStringKey, NSRange, NSString};

pub(crate) use super::state::State;
use super::{access, bar, events, ime, links, pointer};
use crate::surface::{Count, FindStep};

define_class!(
    // SAFETY: NSView may be subclassed; the view is used on the main thread only, and holds
    // no Drop of its own (its ivars are dropped by objc2).
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LlxTermView"]
    #[ivars = Mutex<State>]
    pub(crate) struct TermView;

    impl TermView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            !self.state().is_some_and(|state| state.hushed)
        }

        // The first click in a window that is not the key one reaches the terminal.
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(becomeFirstResponder))]
        fn become_first_responder(&self) -> bool {
            events::focused(self, true);
            true
        }

        #[unsafe(method(resignFirstResponder))]
        fn resign_first_responder(&self) -> bool {
            events::focused(self, false);
            true
        }

        // The grid follows the size the window's layout places it at (`place`); a size the
        // system gives it meanwhile only keeps the cursor's row in view while the grid is held.
        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: CGSize) {
            // SAFETY: NSView's own takes the size, a CGSize.
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.lifted();
        }

        // The person let go of the window's edge (while it is held, the window says it is in
        // a live resize).
        #[unsafe(method(viewDidEndLiveResize))]
        fn did_end_live_resize(&self) {
            // SAFETY: NSView's own, which takes nothing.
            let _: () = unsafe { msg_send![super(self), viewDidEndLiveResize] };
            self.edge_let_go();
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        fn backing_changed(&self) {
            self.restyle();
        }

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn appearance_changed(&self) {
            self.restyle();
        }

        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: CGPoint) -> *mut AnyObject {
            if self.state().is_some_and(|state| state.hushed) {
                return std::ptr::null_mut();
            }
            // SAFETY: NSView's own answers the view under a point, or nil.
            unsafe { msg_send![super(self), hitTest: point] }
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            events::key_down(self, event);
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            events::key_up(self, event);
        }

        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            events::flags_changed(self, event);
        }

        // The window's own shortcuts are its menu's: none is taken here first.
        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, _event: &NSEvent) -> bool {
            false
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            pointer::mouse(self, event, pointer::Pointer::Down);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            pointer::mouse(self, event, pointer::Pointer::Dragged);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            pointer::mouse(self, event, pointer::Pointer::Up);
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            pointer::mouse(self, event, pointer::Pointer::Moved);
        }

        #[unsafe(method(cursorUpdate:))]
        fn cursor_update(&self, _event: &NSEvent) {
            links::shape(self);
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            bar::pointer_in(self, true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            bar::pointer_in(self, false);
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_down(&self, event: &NSEvent) {
            pointer::mouse(self, event, pointer::Pointer::Down);
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_dragged(&self, event: &NSEvent) {
            pointer::mouse(self, event, pointer::Pointer::Dragged);
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_up(&self, event: &NSEvent) {
            pointer::mouse(self, event, pointer::Pointer::Up);
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_down(&self, event: &NSEvent) {
            pointer::right_down(self, event);
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            pointer::wheel(self, event);
        }

        #[unsafe(method(copy:))]
        fn copy(&self, _sender: Option<&AnyObject>) {
            events::copy(self);
        }

        #[unsafe(method(paste:))]
        fn paste(&self, _sender: Option<&AnyObject>) {
            events::paste(self);
        }

        #[unsafe(method(selectAll:))]
        fn select_all(&self, _sender: Option<&AnyObject>) {
            events::select_all(self);
        }

        // A frame of the display while a selection is dragged past an edge.
        #[unsafe(method(dragFrame:))]
        fn drag_frame(&self, _link: Option<&AnyObject>) {
            pointer::drag_frame(self);
        }

        // To VoiceOver, a text area of the screen's lines (`access.rs`).
        #[unsafe(method(isAccessibilityElement))]
        fn is_accessibility_element(&self) -> bool {
            true
        }

        #[unsafe(method_id(accessibilityRole))]
        fn accessibility_role(&self) -> Retained<NSString> {
            access::role()
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn accessibility_label(&self) -> Retained<NSString> {
            access::label()
        }

        #[unsafe(method_id(accessibilityValue))]
        fn accessibility_value(&self) -> Retained<NSString> {
            access::value(self)
        }

        #[unsafe(method(accessibilityNumberOfCharacters))]
        fn accessibility_characters(&self) -> isize {
            access::characters(self)
        }

        #[unsafe(method(accessibilityVisibleCharacterRange))]
        fn accessibility_visible(&self) -> NSRange {
            access::visible(self)
        }

        #[unsafe(method(accessibilitySelectedTextRange))]
        fn accessibility_selected_range(&self) -> NSRange {
            access::selected_range(self)
        }

        #[unsafe(method_id(accessibilitySelectedText))]
        fn accessibility_selected_text(&self) -> Retained<NSString> {
            access::selected_text(self)
        }

        #[unsafe(method(accessibilityInsertionPointLineNumber))]
        fn accessibility_insertion_line(&self) -> isize {
            access::insertion_line(self)
        }

        #[unsafe(method(accessibilityLineForIndex:))]
        fn accessibility_line_for(&self, index: isize) -> isize {
            access::line_for(self, index)
        }

        #[unsafe(method(accessibilityRangeForLine:))]
        fn accessibility_range_for(&self, line: isize) -> NSRange {
            access::range_for(self, line)
        }

        #[unsafe(method_id(accessibilityStringForRange:))]
        fn accessibility_string_for(&self, range: NSRange) -> Retained<NSString> {
            access::string_for(self, range)
        }

        #[unsafe(method_id(accessibilityAttributedStringForRange:))]
        fn accessibility_attributed_for(&self, range: NSRange) -> Retained<NSAttributedString> {
            access::attributed_for(self, range)
        }

        #[unsafe(method(accessibilityFrameForRange:))]
        fn accessibility_frame_for(&self, range: NSRange) -> CGRect {
            access::frame_for(self, range)
        }

        // The window became the key one, or stopped being it.
        #[unsafe(method(windowKeyChanged:))]
        fn window_key_changed(&self, _note: Option<&AnyObject>) {
            events::window_key_changed(self);
        }
    }

    // SAFETY: an NSView subclass is an NSObject, and conforms to its protocol as NSView does.
    unsafe impl NSObjectProtocol for TermView {}

    // SAFETY: each method below has the selector, the argument types and the return type the
    // protocol declares, and is called on the main thread, as AppKit calls an input client.
    unsafe impl NSTextInputClient for TermView {
        #[unsafe(method(insertText:replacementRange:))]
        fn insert_text(&self, string: &AnyObject, _range: NSRange) {
            ime::insert(self, string);
        }

        #[unsafe(method(doCommandBySelector:))]
        fn do_command(&self, _selector: Sel) {}

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        fn set_marked(&self, string: &AnyObject, _selected: NSRange, _range: NSRange) {
            ime::mark(self, string);
        }

        #[unsafe(method(unmarkText))]
        fn unmark(&self) {
            ime::unmark(self);
        }

        #[unsafe(method(selectedRange))]
        fn selected_range(&self) -> NSRange {
            ime::selected(self)
        }

        #[unsafe(method(markedRange))]
        fn marked_range(&self) -> NSRange {
            ime::marked_range(self)
        }

        #[unsafe(method(hasMarkedText))]
        fn has_marked(&self) -> bool {
            self.state().is_some_and(|state| !state.marked.is_empty())
        }

        #[unsafe(method_id(attributedSubstringForProposedRange:actualRange:))]
        fn substring(&self, _range: NSRange, _actual: *mut NSRange) -> Option<Retained<NSAttributedString>> {
            None
        }

        #[unsafe(method_id(validAttributesForMarkedText))]
        fn valid_attributes(&self) -> Retained<NSArray<NSAttributedStringKey>> {
            NSArray::new()
        }

        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        fn first_rect(&self, _range: NSRange, _actual: *mut NSRange) -> CGRect {
            ime::cursor_rect(self)
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn index_for_point(&self, _point: CGPoint) -> usize {
            0
        }
    }
);

impl TermView {
    /// What the view keeps, held for a moment; none while it is held already (a call that came
    /// back into the view from the system while it worked). The view lives on the main thread
    /// alone, so the lock never waits: it only tells such a call to leave the state alone.
    pub(super) fn state(&self) -> Option<MutexGuard<'_, State>> {
        self.ivars().try_lock().ok()
    }

    /// A step of a find in the terminal's text, its matches drawn: where it is and how many.
    pub(crate) fn find(&self, step: &FindStep) -> Count {
        let Some(surface) = events::surface(self) else {
            return Count::default();
        };
        let count = surface.find(step);
        // A recount moves nothing and lights nothing new: the view draws as output comes.
        if *step != FindStep::Again {
            self.redraw(false);
        }
        count
    }

    /// What the program asked to put on the pasteboard since this was last asked, and whether
    /// it rang the bell: the window decides where either goes.
    pub(crate) fn told(&self) -> (Option<String>, bool) {
        let surface = self.state().map(|state| Arc::clone(&state.surface));
        surface.map_or((None, false), |surface| (surface.copied(), surface.rang()))
    }

    /// Under the window's dialog (`on`), the view takes no click, no key and no pointer.
    pub(crate) fn hush(&self, on: bool) {
        if let Some(mut state) = self.state() {
            state.hushed = on;
        }
    }
}
