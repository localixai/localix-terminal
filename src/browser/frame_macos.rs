//! The glass on macOS, through AppKit's own messages (no bindings beyond what the WebView links).
//! The material sits in the window's frame view, below everything, so it backs the titlebar and
//! the pages alike; a panel's coat sits just above it.
//!
//! The ground is an NSVisualEffectView on purpose, not an NSGlassEffectView: Liquid Glass is the
//! system's layer for controls floating over content, and this ground lies behind the terminals,
//! the board and Settings, which are content (and its pages are siblings of the ground, where
//! glass only treats its own content view). Glass belongs to small floating pieces, never to
//! this layer, so this is not a free upgrade to make.

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use tao::platform::macos::WindowBuilderExtMacOS;
use tao::window::{Window, WindowBuilder};

use super::super::capture::{CGRect, CGSize};

/// NSAutoresizingMaskOptions: width and height follow the frame view.
const SIZABLE: usize = 2 | 16;
/// NSWindowOrderingMode.
const BELOW: isize = -1;
const ABOVE: isize = 1;

/// A window that shows what is behind it, with its titlebar clear over the material. The strip
/// is the titlebar, as in a browser: the pages fill the window from its top, the strip beside the
/// window's buttons, and the title is hidden (the strip names the tabs).
pub(super) fn clear(builder: WindowBuilder) -> WindowBuilder {
    builder
        .with_transparent(true)
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .with_title_hidden(true)
}

/// An empty toolbar of the compact unified style, as Safari's compact tabs have: AppKit makes the titlebar that
/// tall and centres the window's buttons on it itself, through every resize. The strip draws
/// the toolbar's contents under it (the content fills the window).
pub(super) fn toolbar(window: &Window) {
    /// NSWindowToolbarStyle: automatic 0, expanded 1, preference 2, unified 3, unified compact 4
    /// (38 points tall, `strip::BAR`, the buttons centred on it).
    const UNIFIED: isize = 4;
    let object = crate::browser::appkit::native(window);
    let name = crate::browser::appkit::text("desk.bar");
    // SAFETY: `alloc` on NSToolbar answers an uninitialised instance.
    let made: Allocated<AnyObject> = unsafe { msg_send![class!(NSToolbar), alloc] };
    // SAFETY: a toolbar is initialised with an NSString identifier and answers itself, retained.
    let toolbar: Retained<AnyObject> = unsafe { msg_send![made, initWithIdentifier: &*name] };
    // SAFETY: a toolbar takes a BOOL: no configuration is written to the person's defaults.
    let _: () = unsafe { msg_send![&*toolbar, setAutosavesConfiguration: Bool::NO] };
    // SAFETY: as above: it is empty, with no Customize Toolbar to offer on the titlebar.
    let _: () = unsafe { msg_send![&*toolbar, setAllowsUserCustomization: Bool::NO] };
    // SAFETY: a toolbar takes a BOOL: no line under it, the strip is one sheet with the page.
    // (Older systems read this; since macOS 15 the window's separator style does.)
    let _: () = unsafe { msg_send![&*toolbar, setShowsBaselineSeparator: Bool::NO] };
    // SAFETY: a window takes an NSTitlebarSeparatorStyle (an NSInteger): none.
    let _: () = unsafe { msg_send![object, setTitlebarSeparatorStyle: 1isize] };
    // SAFETY: a window takes a toolbar, which it retains.
    let _: () = unsafe { msg_send![object, setToolbar: &*toolbar] };
    // SAFETY: a window takes an NSWindowToolbarStyle (an NSInteger).
    let _: () = unsafe { msg_send![object, setToolbarStyle: UNIFIED] };
}

/// Full screen has no titlebar of its own to draw: the system gives a window with a toolbar a
/// solid bar of it over the top of the content, which would hide the strip. So the toolbar is let
/// go while the window is full screen (the strip is the top, the buttons come over it when the
/// pointer reaches the top edge) and made again when it leaves.
pub(super) fn chrome(window: &Window, full: bool) {
    let object = crate::browser::appkit::native(window);
    // SAFETY: a window answers its toolbar, or nil.
    let has: *mut AnyObject = unsafe { msg_send![object, toolbar] };
    match (full, has.is_null()) {
        (true, false) => {
            let none: Option<&AnyObject> = None;
            // SAFETY: a window takes a toolbar, or nil to have none.
            let _: () = unsafe { msg_send![object, setToolbar: none] };
        }
        (false, true) => toolbar(window),
        _ => {}
    }
}

/// Whether the person asked for less transparency (Accessibility, Display).
pub(super) fn reduced() -> bool {
    // SAFETY: `sharedWorkspace` is NSWorkspace's class method, returning the shared instance.
    let workspace: Retained<AnyObject> = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
    // SAFETY: an NSWorkspace answers this property with a BOOL.
    let reduced: Bool =
        unsafe { msg_send![&*workspace, accessibilityDisplayShouldReduceTransparency] };
    reduced.as_bool()
}

/// Whether the person asked for more contrast (Accessibility, Display).
pub(super) fn increased() -> bool {
    // SAFETY: `sharedWorkspace` is NSWorkspace's class method, returning the shared instance.
    let workspace: Retained<AnyObject> = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
    // SAFETY: an NSWorkspace answers this property with a BOOL.
    let more: Bool = unsafe { msg_send![&*workspace, accessibilityDisplayShouldIncreaseContrast] };
    more.as_bool()
}

/// What names our material among the frame view's own (the system puts materials there too).
fn name() -> Retained<AnyObject> {
    crate::browser::appkit::text("desk.glass")
}

/// What names the coat over it.
fn coat_name() -> Retained<AnyObject> {
    crate::browser::appkit::text("desk.coat")
}

/// The view that holds the titlebar and the content.
fn frame_view(window: &Window) -> Option<Retained<AnyObject>> {
    let window = crate::browser::appkit::native(window);
    // SAFETY: an NSWindow answers its content view, or nil, which the `Option` takes.
    let content: Option<Retained<AnyObject>> = unsafe { msg_send![window, contentView] };
    let content = content?;
    // SAFETY: an NSView answers `superview` with a view or nil.
    unsafe { msg_send![&*content, superview] }
}

/// A window's material, and the coat over it. Kept, so a new tint changes these two and never
/// adds another.
pub(super) struct Glass {
    effect: Retained<AnyObject>,
    coat: Retained<AnyObject>,
}

impl Glass {
    /// The material under everything in `window`, and its coat.
    pub(super) fn under(window: &Window) -> Option<Self> {
        let frame = frame_view(window)?;
        // SAFETY: an NSView answers `bounds` with an NSRect, which `CGRect` lays out.
        let bounds: CGRect = unsafe { msg_send![&*frame, bounds] };
        let effect = view(class!(NSVisualEffectView), bounds, &name());
        // SAFETY: an NSVisualEffectView takes an NSInteger blending mode: behind the window,
        // the desktop, blurred.
        let _: () = unsafe { msg_send![&*effect, setBlendingMode: 0isize] };
        // Active, always: a window the person watches while another app is key (an agent working
        // in it) stays glass, as it is when it is the key one; an inactive material stops
        // sampling and paints flat grey.
        // SAFETY: an NSVisualEffectView takes an NSInteger state.
        let _: () = unsafe { msg_send![&*effect, setState: 1isize] };
        let none: Option<&AnyObject> = None;
        // SAFETY: `frame` and `effect` are live views; a nil sibling puts it below them all.
        let _: () = unsafe {
            msg_send![&*frame, addSubview: &*effect, positioned: BELOW, relativeTo: none]
        };
        let coat = view(class!(NSBox), bounds, &coat_name());
        // A custom box: a fill and nothing else, no border and no title.
        // SAFETY: an NSBox takes an NSBoxType (an NSUInteger); 4 is custom.
        let _: () = unsafe { msg_send![&*coat, setBoxType: 4usize] };
        // SAFETY: an NSBox takes an NSTitlePosition (an NSUInteger); 0 is none.
        let _: () = unsafe { msg_send![&*coat, setTitlePosition: 0usize] };
        // SAFETY: an NSBox takes a CGFloat border width.
        let _: () = unsafe { msg_send![&*coat, setBorderWidth: 0.0f64] };
        // SAFETY: `frame`, `coat` and `effect` are live views, `effect` already a subview.
        let _: () = unsafe {
            msg_send![&*frame, addSubview: &*coat, positioned: ABOVE, relativeTo: &*effect]
        };
        Some(Self { effect, coat })
    }

    /// The material `material`, under the coat `[red, green, blue]` at `alpha`. A coat too thin
    /// to see is hidden, and a material under one too thick to see through is: a hidden view
    /// costs no compositing, and a material nobody sees would still sample the desktop.
    pub(super) fn tint(&self, material: isize, [red, green, blue]: [u8; 3], alpha: f64) {
        let part = |value: u8| f64::from(value) / 255.0;
        // SAFETY: an NSVisualEffectView takes an NSInteger material.
        let _: () = unsafe { msg_send![&*self.effect, setMaterial: material] };
        // SAFETY: a view takes a BOOL `hidden`.
        let _: () = unsafe { msg_send![&*self.coat, setHidden: Bool::new(alpha <= 0.02)] };
        // SAFETY: as above.
        let _: () = unsafe { msg_send![&*self.effect, setHidden: Bool::new(alpha >= 0.99)] };
        // SAFETY: NSColor's class method takes four CGFloats and returns a colour.
        let colour: Retained<AnyObject> = unsafe {
            msg_send![class!(NSColor),
                colorWithSRGBRed: part(red), green: part(green), blue: part(blue), alpha: alpha]
        };
        // SAFETY: an NSBox takes an NSColor fill, which it retains.
        let _: () = unsafe { msg_send![&*self.coat, setFillColor: &*colour] };
    }
}

/// A new view of `class`, as large as `bounds` and sized with its parent, named `name`.
fn view(class: &objc2::runtime::AnyClass, bounds: CGRect, name: &AnyObject) -> Retained<AnyObject> {
    // SAFETY: `class` is an NSView subclass, and `new` returns a retained instance.
    let view: Retained<AnyObject> = unsafe { msg_send![class, new] };
    // SAFETY: an NSView takes an NSRect frame, which `CGRect` lays out.
    let _: () = unsafe { msg_send![&*view, setFrame: bounds] };
    // SAFETY: an NSView takes an NSAutoresizingMaskOptions (an NSUInteger).
    let _: () = unsafe { msg_send![&*view, setAutoresizingMask: SIZABLE] };
    // SAFETY: an NSView takes an NSString identifier, which it copies.
    let _: () = unsafe { msg_send![&*view, setIdentifier: name] };
    view
}

/// Our materials in `window`'s frame view, the first one's state, whether it is opaque, the
/// shape its resizing keeps, and whether a coat is over them.
pub(super) fn dressed(window: &Window) -> (usize, isize, bool, f64, bool) {
    let native = crate::browser::appkit::native(window);
    // SAFETY: an NSWindow answers `isOpaque` with a BOOL.
    let opaque: Bool = unsafe { msg_send![native, isOpaque] };
    // SAFETY: an NSWindow answers `contentAspectRatio` with an NSSize, which `CGSize` lays out.
    let shape: CGSize = unsafe { msg_send![native, contentAspectRatio] };
    let aspect = if shape.height > 0.0 {
        shape.width / shape.height
    } else {
        0.0
    };
    let Some(frame) = frame_view(window) else {
        return (0, -1, opaque.as_bool(), aspect, false);
    };
    let (mut found, mut coated) = ((0, -1), false);
    // SAFETY: an NSView answers `subviews` with an NSArray.
    let views: Retained<AnyObject> = unsafe { msg_send![&*frame, subviews] };
    // SAFETY: an NSArray answers `count` with an NSUInteger.
    let count: usize = unsafe { msg_send![&*views, count] };
    for at in 0..count {
        // SAFETY: `at` is below the array's count, read just above on the same thread.
        let view: Retained<AnyObject> = unsafe { msg_send![&*views, objectAtIndex: at] };
        // SAFETY: an NSView answers `identifier` with an NSString or nil.
        let named: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, identifier] };
        let is = |wanted: &AnyObject| -> bool {
            named.as_ref().is_some_and(|named| {
                // SAFETY: both are NSStrings; the comparison answers a BOOL.
                let same: Bool = unsafe { msg_send![&**named, isEqualToString: wanted] };
                same.as_bool()
            })
        };
        coated |= is(&coat_name());
        if is(&name()) {
            // SAFETY: a view named ours is the NSVisualEffectView made above; `state` answers
            // an NSInteger.
            let state: isize = unsafe { msg_send![&*view, state] };
            found = (found.0 + 1, if found.0 == 0 { state } else { found.1 });
        }
    }
    (found.0, found.1, opaque.as_bool(), aspect, coated)
}

/// What a window remembers of full screen: whether it was in it at the last event, the frame it
/// had outside it (outer position, inner size), and until when that frame is put back (tao zooms
/// a window it took for zoomed on the way in, as it comes out; the zoom lands a moment later).
#[derive(Default)]
pub(super) struct Screen {
    was: bool,
    frame: Option<(tao::dpi::PhysicalPosition<i32>, tao::dpi::PhysicalSize<u32>)>,
    until: Option<std::time::Instant>,
}

impl Screen {
    /// The frame the window has now, when it is not full screen: the one to come back to.
    pub(super) fn remember(&mut self, window: &Window) {
        if window.fullscreen().is_none()
            && let Ok(at) = window.outer_position()
        {
            self.frame = Some((at, window.inner_size()));
        }
    }

    /// An event of the window, `full` or not: the frame is kept while it is the person's, and put
    /// back, for a moment, after full screen ends. Whether it was full before.
    pub(super) fn step(&mut self, window: &Window, full: bool) -> bool {
        let was = self.was;
        self.was = full;
        if was && !full {
            self.until = Some(std::time::Instant::now() + std::time::Duration::from_millis(1500));
        }
        if full {
            return was;
        }
        if self.until.is_some_and(|until| std::time::Instant::now() < until) {
            if let Some((at, size)) = self.frame {
                let now = (window.outer_position().ok(), window.inner_size());
                if now != (Some(at), size) {
                    window.set_inner_size(size);
                    window.set_outer_position(at);
                }
            }
        } else {
            self.until = None;
            self.remember(window);
        }
        was
    }
}
