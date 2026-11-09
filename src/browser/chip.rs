//! The tab held in a side's list, drawn where the pointer is (macOS), as a window that draws
//! itself whole draws a tab dragged in it: over the list, the pages, another window, the desktop,
//! not kept to the side's view. It is a small clear window over every other, showing the picture
//! of the card the side lifted (the row itself, as the list draws it), the pointer holding it
//! where it took it. It takes no click and no key.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use block2::RcBlock;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use wry::WebViewExtMacOS;

use super::appkit::masks::{
    BORDERLESS, CAN_JOIN_ALL_SPACES, FULL_SCREEN_AUXILIARY, HEIGHT_SIZABLE, IGNORES_CYCLE,
    NONACTIVATING_PANEL, POP_UP_MENU_LEVEL, WIDTH_SIZABLE,
};
use super::capture::{CGPoint, CGRect, CGSize};

/// NSBackingStoreBuffered.
const BUFFERED: usize = 2;
/// NSWindowCollectionBehavior: on every Space, over a full-screen app, out of window cycling.
const EVERYWHERE: usize = CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY | IGNORES_CYCLE;
/// NSImageScaleAxesIndependently: the picture fills the window, which is its size.
const FILL: usize = 1;
/// The picture's view follows the window's size.
const STRETCH: usize = WIDTH_SIZABLE | HEIGHT_SIZABLE;
/// The card's corners, as the list draws them (points).
const ROUND: f64 = 10.0;

pub(super) struct Chip {
    panel: Retained<AnyObject>,
    /// Its size, in points.
    size: (f64, f64),
    /// Where in it the pointer holds it.
    grab: (f64, f64),
    /// Whether it is to be seen: the picture comes later than the window, and shows only if so.
    wanted: Arc<AtomicBool>,
    /// Whether its picture is in.
    drawn: Arc<AtomicBool>,
}

impl Chip {
    /// The card the side's page `side` lifted, at `rect` of the page (points), held `grab` from
    /// its top left, the page's view `inner` points tall: its picture is taken, then shown at the
    /// pointer, and the page told (`CH()`) to stop drawing its own.
    pub(super) fn lift(
        side: &wry::WebView,
        rect: (f64, f64, f64, f64),
        grab: (f64, f64),
        inner: Option<f64>,
    ) -> Self {
        let (_, _, width, height) = rect;
        let frame = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize { width, height },
        };
        let panel = panel(frame);
        let picture = picture(frame);
        // SAFETY: a window takes an NSView as its content view, which it retains.
        let _: () = unsafe { msg_send![&*panel, setContentView: &*picture] };
        let chip = Self {
            panel,
            size: (width, height),
            grab,
            wanted: Arc::new(AtomicBool::new(true)),
            drawn: Arc::new(AtomicBool::new(false)),
        };
        chip.take(side, rect, inner, picture);
        chip
    }

    /// Take the card's picture at `rect` of `side`'s page (`inner` points tall), put it in
    /// `picture`, and show the chip if it is still wanted.
    fn take(
        &self,
        side: &wry::WebView,
        rect: (f64, f64, f64, f64),
        inner: Option<f64>,
        picture: Retained<AnyObject>,
    ) {
        let (x, y, width, height) = rect;
        let view = side.webview();
        // SAFETY: an NSView answers its bounds, an NSRect, which `CGRect` lays out.
        let bounds: CGRect = unsafe { msg_send![&*view, bounds] };
        // The view's points run from its own top, the page's from where its content begins: a
        // view taller than the page's content begins that much above it.
        let above = inner.map_or(0.0, |inner| (bounds.size.height - inner).max(0.0));
        let y = y + above;
        // SAFETY: `new` on WKSnapshotConfiguration answers a retained configuration.
        let configuration: Retained<AnyObject> =
            unsafe { msg_send![class!(WKSnapshotConfiguration), new] };
        let area = CGRect {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        };
        // SAFETY: a snapshot configuration takes an NSRect, which `CGRect` lays out.
        let _: () = unsafe { msg_send![&*configuration, setRect: area] };
        // SAFETY: a snapshot configuration takes a BOOL.
        let _: () = unsafe { msg_send![&*configuration, setAfterScreenUpdates: Bool::YES] };
        let panel = self.panel.clone();
        let (wanted, drawn) = (Arc::clone(&self.wanted), Arc::clone(&self.drawn));
        let page = view.clone();
        let done = RcBlock::new(move |image: *mut AnyObject, _error: *mut AnyObject| {
            if image.is_null() {
                return;
            }
            // SAFETY: WebKit hands a live NSImage for the length of the callback; the image view
            // retains it.
            let _: () = unsafe { msg_send![&*picture, setImage: image] };
            drawn.store(true, Ordering::SeqCst);
            if wanted.load(Ordering::SeqCst) {
                front(&panel);
            }
            let script = super::appkit::text("CH()");
            let none: *const AnyObject = std::ptr::null();
            // SAFETY: a WKWebView takes a script (NSString) and a completion handler, nil here.
            let _: () =
                unsafe { msg_send![&*page, evaluateJavaScript: &*script, completionHandler: none] };
        });
        // SAFETY: a WKWebView takes a configuration and a block of (NSImage, NSError), which it
        // copies and calls once on the main thread.
        let _: () = unsafe {
            msg_send![
                &*view,
                takeSnapshotWithConfiguration: &*configuration,
                completionHandler: &*done
            ]
        };
    }

    /// The pointer is at `x`, `y` of the screen (points, from the top left of the main screen):
    /// the chip is where the pointer holds it.
    pub(super) fn follow(&self, x: f64, y: f64) {
        let top_left = CGPoint {
            x: x - self.grab.0,
            y: main_height() - (y - self.grab.1),
        };
        // SAFETY: a window takes an NSPoint, its top left from the main screen's bottom left.
        let _: () = unsafe { msg_send![&*self.panel, setFrameTopLeftPoint: top_left] };
    }

    /// The chip seen (the tab is carried as a card) or not (a window carries it).
    pub(super) fn show(&self, shown: bool) {
        self.wanted.store(shown, Ordering::SeqCst);
        if !self.drawn.load(Ordering::SeqCst) {
            return;
        }
        if shown {
            front(&self.panel);
        } else {
            hide(&self.panel);
        }
    }

    /// Its height, in points: the empty place a list opens for it is as tall.
    pub(super) fn height(&self) -> f64 {
        self.size.1
    }
}

impl Drop for Chip {
    fn drop(&mut self) {
        self.wanted.store(false, Ordering::SeqCst);
        hide(&self.panel);
        // SAFETY: a window takes `close`; it is not released when closed (`dress`).
        let _: () = unsafe { msg_send![&*self.panel, close] };
    }
}

/// The chip's window, `frame` big: clear but for its picture, with the system's shadow, over
/// every window and Space, taking no click.
fn panel(frame: CGRect) -> Retained<AnyObject> {
    // SAFETY: `alloc` on NSPanel answers an uninitialised instance.
    let alloc: Allocated<AnyObject> = unsafe { msg_send![class!(NSPanel), alloc] };
    // SAFETY: NSPanel's designated initialiser takes a rect, a style mask, a backing store type
    // and a BOOL, and answers the panel, retained.
    let panel: Retained<AnyObject> = unsafe {
        msg_send![
            alloc,
            initWithContentRect: frame,
            styleMask: BORDERLESS | NONACTIVATING_PANEL,
            backing: BUFFERED,
            defer: Bool::NO
        ]
    };
    // SAFETY: a window takes a BOOL.
    let _: () = unsafe { msg_send![&*panel, setReleasedWhenClosed: Bool::NO] };
    // SAFETY: a window takes a BOOL.
    let _: () = unsafe { msg_send![&*panel, setOpaque: Bool::NO] };
    // SAFETY: `clearColor` answers a shared NSColor.
    let clear: *mut AnyObject = unsafe { msg_send![class!(NSColor), clearColor] };
    // SAFETY: a window takes an NSColor, which it retains.
    let _: () = unsafe { msg_send![&*panel, setBackgroundColor: clear] };
    // SAFETY: a window takes a BOOL.
    let _: () = unsafe { msg_send![&*panel, setHasShadow: Bool::YES] };
    // SAFETY: a window takes a BOOL.
    let _: () = unsafe { msg_send![&*panel, setIgnoresMouseEvents: Bool::YES] };
    // SAFETY: a window takes a BOOL.
    let _: () = unsafe { msg_send![&*panel, setHidesOnDeactivate: Bool::NO] };
    // SAFETY: a window takes an NSWindowAnimationBehavior (an NSInteger); 2 is none: a card
    // that comes and goes with the pointer never fades or zooms in and out.
    let _: () = unsafe { msg_send![&*panel, setAnimationBehavior: 2isize] };
    // SAFETY: a window takes an NSWindowLevel.
    let _: () = unsafe { msg_send![&*panel, setLevel: POP_UP_MENU_LEVEL] };
    // SAFETY: a window takes an NSWindowCollectionBehavior.
    let _: () = unsafe { msg_send![&*panel, setCollectionBehavior: EVERYWHERE] };
    panel
}

/// The view that shows the card's picture, filling `frame`, its corners rounded as the card's.
fn picture(frame: CGRect) -> Retained<AnyObject> {
    // SAFETY: `new` on NSImageView answers a retained view.
    let picture: Retained<AnyObject> = unsafe { msg_send![class!(NSImageView), new] };
    // SAFETY: a view takes an NSRect, which `CGRect` lays out.
    let _: () = unsafe { msg_send![&*picture, setFrame: frame] };
    // SAFETY: a view takes an autoresizing mask.
    let _: () = unsafe { msg_send![&*picture, setAutoresizingMask: STRETCH] };
    // SAFETY: an image view takes an NSImageScaling.
    let _: () = unsafe { msg_send![&*picture, setImageScaling: FILL] };
    // SAFETY: a view takes a BOOL.
    let _: () = unsafe { msg_send![&*picture, setWantsLayer: Bool::YES] };
    // SAFETY: a layer-backed view answers its CALayer, or nil.
    let layer: *mut AnyObject = unsafe { msg_send![&*picture, layer] };
    if !layer.is_null() {
        // SAFETY: a CALayer takes a CGFloat.
        let _: () = unsafe { msg_send![layer, setCornerRadius: ROUND] };
        // SAFETY: a CALayer takes a BOOL.
        let _: () = unsafe { msg_send![layer, setMasksToBounds: Bool::YES] };
    }
    picture
}

/// The window over every other, its shadow drawn for the picture in it.
fn front(panel: &AnyObject) {
    // SAFETY: a window takes `orderFrontRegardless`.
    let _: () = unsafe { msg_send![panel, orderFrontRegardless] };
    // SAFETY: a window takes `invalidateShadow`.
    let _: () = unsafe { msg_send![panel, invalidateShadow] };
}

/// The window out of sight.
fn hide(panel: &AnyObject) {
    let none: *const AnyObject = std::ptr::null();
    // SAFETY: a window takes `orderOut:` with a sender, nil here.
    let _: () = unsafe { msg_send![panel, orderOut: none] };
}

/// The main screen's height, in points: the screen's points run up from its bottom left, the
/// page's down from its top left.
fn main_height() -> f64 {
    // SAFETY: `screens` answers an NSArray of NSScreen, the main one (with the menu bar) first.
    let screens: *mut AnyObject = unsafe { msg_send![class!(NSScreen), screens] };
    if screens.is_null() {
        return 0.0;
    }
    // SAFETY: an NSArray answers its count.
    let count: usize = unsafe { msg_send![screens, count] };
    if count == 0 {
        return 0.0;
    }
    // SAFETY: the index is within the array.
    let main: *mut AnyObject = unsafe { msg_send![screens, objectAtIndex: 0usize] };
    // SAFETY: an NSScreen answers its frame, an NSRect, which `CGRect` lays out.
    let frame: CGRect = unsafe { msg_send![main, frame] };
    frame.size.height
}
