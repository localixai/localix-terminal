//! The window's views moved as Core Animation plays them (macOS; `glide.rs` has the geometry).
//! Where each view stands is read before a layout and again after it; a view that moved has a
//! spring added to its own layer, from where it stood to nothing, which the system adds to the
//! place and the bounds the layout gave it (and to how far what it holds is raised in it). The springs still playing go on, so the view is
//! taken on from where it is drawn and as fast as it goes. Nothing is laid out, drawn or timed
//! by the app on the way: the view was laid out once, its layer clips what it holds to its
//! bounds, and the system plays the rest.

use std::sync::atomic::{AtomicU64, Ordering};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, MessageReceiver};
use objc2::{class, msg_send, sel};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSArray, NSString};
use wry::WebViewExtMacOS;

use super::{Drawn, Frame, Moved, Opens, offset, shut, toward};
use crate::browser::motion;

/// What every animation added here is kept under on its layer, a number after it.
const KEY: &str = "glide.";

/// The key path of how far down what a layer holds is moved as one.
const SHIFT: &str = "sublayerTransform.translation.y";

/// Counts the animations added, so each has a key of its own and none takes another's place.
static ADDED: AtomicU64 = AtomicU64::new(0);

impl Moved<'_> {
    /// The view as AppKit sees it.
    fn view(&self) -> Option<Retained<AnyObject>> {
        match self {
            Self::Term(view) => retained(view),
            Self::Page(page) => {
                let view = page.webview();
                let any: &AnyObject = &view;
                retained(any)
            }
        }
    }
}

/// A live object retained once more.
fn retained(object: &AnyObject) -> Option<Retained<AnyObject>> {
    // SAFETY: the object is alive for the borrow; retaining it keeps it alive past it.
    unsafe { Retained::retain(std::ptr::from_ref(object).cast_mut()) }
}

/// One view as it stood before a layout: where, how far down its layer what it holds was
/// moved as one, and whether it was in sight.
struct Stood {
    view: Retained<AnyObject>,
    frame: Frame,
    shift: f64,
    seen: bool,
}

/// Where the views stood before a layout.
pub(in crate::browser) struct Standing(Vec<Stood>);

/// Where `views` stand now, to be read before the layout that moves them.
pub(in crate::browser) fn standing(views: &[Moved<'_>]) -> Standing {
    let stood = |view: Retained<AnyObject>| {
        let (frame, seen) = (frame(&view), seen(&view));
        let shift = layer_of(&view).map_or(0.0, |layer| shift(&layer));
        Stood {
            view,
            frame,
            shift,
            seen,
        }
    };
    Standing(views.iter().filter_map(Moved::view).map(stood).collect())
}

impl Standing {
    /// The layout made: each view in sight that moved goes from where it stood to where it
    /// stands now; the pane that `opens` comes from the edge it lands at, its divider with it,
    /// and fades in. Under Reduce Motion nothing travels: the pane only fades in.
    pub(in crate::browser) fn go(self, opens: Option<&Opens<'_>>) {
        let reduced = crate::browser::appkit::reduced_motion();
        let opening: Vec<Retained<AnyObject>> = opens
            .map(|opens| opens.views.iter().filter_map(Moved::view).collect())
            .unwrap_or_default();
        for stood in &self.0 {
            let view = &*stood.view;
            if !seen(view) {
                continue;
            }
            let after = frame(view);
            let opened = opens.filter(|_| opening.iter().any(|it| same(it, view)));
            match opened {
                // Where it stood and stands is one place: it opened there already.
                Some(_) if stood.seen && stood.frame == after => {}
                Some(opens) => {
                    still(view);
                    if !reduced {
                        spring(view, shut(after, opens.side, from_top(view)), after);
                    }
                    let seconds = if reduced {
                        motion::T_FADE
                    } else {
                        motion::T_IN
                    };
                    fade_in(view, seconds);
                }
                None if stood.seen && !reduced => {
                    spring(view, stood.frame, after);
                    carry_on(view, stood.shift);
                }
                None => {}
            }
        }
        let Some(Opens {
            line: Some((line, by)),
            side,
            ..
        }) = opens
        else {
            return;
        };
        let line = line.webview();
        let line: &AnyObject = &line;
        still(line);
        if !reduced {
            let after = frame(line);
            spring(line, toward(after, *side, *by, from_top(line)), after);
        }
    }
}

fn same(one: &AnyObject, other: &AnyObject) -> bool {
    std::ptr::eq(one, other)
}

/// The view's frame in its parent's points.
fn frame(view: &AnyObject) -> Frame {
    // SAFETY: an NSView answers its frame, a CGRect.
    let frame: CGRect = unsafe { msg_send![view, frame] };
    [
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height,
    ]
}

/// The view's parent, or none.
fn parent(view: &AnyObject) -> Option<Retained<AnyObject>> {
    // SAFETY: an NSView answers its superview, or nil.
    unsafe { msg_send![view, superview] }
}

/// Whether the view's parent counts from its top.
fn from_top(view: &AnyObject) -> bool {
    // SAFETY: an NSView answers whether it counts from the top, a BOOL.
    parent(view).is_some_and(|parent| unsafe { msg_send![&*parent, isFlipped] })
}

/// Whether the view is in sight: not hidden, and some of it within its parent.
fn seen(view: &AnyObject) -> bool {
    // SAFETY: an NSView answers whether it or a view it is in is hidden, a BOOL.
    let hidden: bool = unsafe { msg_send![view, isHiddenOrHasHiddenAncestor] };
    let Some(parent) = parent(view).filter(|_| !hidden) else {
        return false;
    };
    // SAFETY: an NSView answers its bounds, a CGRect.
    let room: CGRect = unsafe { msg_send![&*parent, bounds] };
    let [x, y, wide, tall] = frame(view);
    x < room.origin.x + room.size.width
        && x + wide > room.origin.x
        && y < room.origin.y + room.size.height
        && y + tall > room.origin.y
}

/// How far down its layer what `layer` holds is moved as one (a terminal's rows raised to
/// keep its cursor's row in a pane grown shorter are moved up: under nothing).
fn shift(layer: &AnyObject) -> f64 {
    let path = NSString::from_str(SHIFT);
    // SAFETY: a layer answers a part of a transform of its own by its key path, a number.
    let value: Option<Retained<AnyObject>> = unsafe { msg_send![layer, valueForKeyPath: &*path] };
    // SAFETY: a number answers its value as a double.
    value.map_or(0.0, |value| unsafe { msg_send![&*value, doubleValue] })
}

/// The view's layer, or none before it has one.
fn layer_of(view: &AnyObject) -> Option<Retained<AnyObject>> {
    // SAFETY: an NSView answers its layer, or nil.
    unsafe { msg_send![view, layer] }
}

/// The view's layer drawn from over `before` to over `after`, where the view stands: a spring
/// of its place, one of its bounds' size and, where what it holds is counted from its bottom,
/// one of where they begin, each from that far off to nothing, added to what the layer plays
/// already. What the view holds is cut at its bounds on the way.
fn spring(view: &AnyObject, before: Frame, after: Frame) {
    let Some(layer) = layer_of(view) else {
        return;
    };
    // SAFETY: a layer answers where it is placed by, a CGPoint of its own unit square.
    let anchor: CGPoint = unsafe { msg_send![&*layer, anchorPoint] };
    // SAFETY: and whether what it holds is counted from its top, a BOOL.
    let top: bool = unsafe { msg_send![&*layer, contentsAreFlipped] };
    let far = offset(before, after, (anchor.x, anchor.y), top);
    if far.none() {
        return;
    }
    // SAFETY: an NSView takes whether it clips what it draws to its bounds, a BOOL.
    let _: () = unsafe { msg_send![view, setClipsToBounds: Bool::YES] };
    let point = |(x, y): (f64, f64)| -> Retained<AnyObject> {
        // SAFETY: an NSValue holds a point by value.
        unsafe { msg_send![class!(NSValue), valueWithPoint: CGPoint::new(x, y)] }
    };
    let size = |(wide, tall): (f64, f64)| -> Retained<AnyObject> {
        // SAFETY: an NSValue holds a size by value.
        unsafe { msg_send![class!(NSValue), valueWithSize: CGSize::new(wide, tall)] }
    };
    let none = (0.0, 0.0);
    play(&layer, "position", &point(far.place), &point(none));
    play(&layer, "bounds.size", &size(far.size), &size(none));
    if far.begin != none {
        play(&layer, "bounds.origin", &point(far.begin), &point(none));
    }
}

/// What the view's layer holds drawn from where it lay (`before`, down its layer) to where it
/// lies now, by the spring its edges go by: a terminal's rows, raised to keep its cursor's
/// row in a pane grown shorter, go up with the pane's edge and come down with it, never at
/// once while the edge is still on its way.
fn carry_on(view: &AnyObject, before: f64) {
    let Some(layer) = layer_of(view) else {
        return;
    };
    let far = before - shift(&layer);
    if far.abs() < 0.01 {
        return;
    }
    let number = |value: f64| -> Retained<AnyObject> {
        // SAFETY: NSNumber holds a double.
        unsafe { msg_send![class!(NSNumber), numberWithDouble: value] }
    };
    play(&layer, SHIFT, &number(far), &number(0.0));
}

/// A spring of `layer`'s `path` from `from` to `to`, added to the value the layer has, as the
/// token sheet's spring that makes room moves (`motion::ROOM`).
fn play(layer: &AnyObject, path: &str, from: &AnyObject, to: &AnyObject) {
    let spring = motion::spring(motion::ROOM);
    let path = NSString::from_str(path);
    // SAFETY: CASpringAnimation answers an autoreleased animation of the key path, retained.
    let animation: Retained<AnyObject> =
        unsafe { msg_send![class!(CASpringAnimation), animationWithKeyPath: &*path] };
    // SAFETY: a spring animation takes its mass, a CGFloat.
    let _: () = unsafe { msg_send![&*animation, setMass: spring.mass] };
    // SAFETY: its stiffness, a CGFloat.
    let _: () = unsafe { msg_send![&*animation, setStiffness: spring.stiffness] };
    // SAFETY: its damping, a CGFloat.
    let _: () = unsafe { msg_send![&*animation, setDamping: spring.damping] };
    // SAFETY: how long it plays, a CFTimeInterval.
    let _: () = unsafe { msg_send![&*animation, setDuration: spring.duration] };
    // SAFETY: the value it plays from, an object it retains.
    let _: () = unsafe { msg_send![&*animation, setFromValue: from] };
    // SAFETY: and the one it plays to.
    let _: () = unsafe { msg_send![&*animation, setToValue: to] };
    // SAFETY: and whether its values are added to the layer's own, a BOOL.
    let _: () = unsafe { msg_send![&*animation, setAdditive: Bool::YES] };
    add(layer, &animation);
}

/// `animation` played on `layer` under a key of its own.
fn add(layer: &AnyObject, animation: &AnyObject) {
    let key = format!("{KEY}{}", ADDED.fetch_add(1, Ordering::Relaxed));
    let key = NSString::from_str(&key);
    // SAFETY: a layer takes an animation (which it copies) under a key.
    let _: () = unsafe { msg_send![layer, addAnimation: animation, forKey: &*key] };
}

/// The keys of what was added here that `layer` still plays.
fn playing(layer: &AnyObject) -> Vec<Retained<NSString>> {
    // SAFETY: a layer answers the keys of its animations, an array of strings or nil.
    let keys: Option<Retained<NSArray<NSString>>> = unsafe { msg_send![layer, animationKeys] };
    let ours = |key: &Retained<NSString>| key.to_string().starts_with(KEY);
    keys.map(|keys| keys.iter().filter(ours).collect())
        .unwrap_or_default()
}

/// What was added here to the view's layer let go of: it is drawn where it stands.
fn still(view: &AnyObject) {
    let Some(layer) = layer_of(view) else {
        return;
    };
    for key in playing(&layer) {
        // SAFETY: a layer lets go of the animation it plays under a key.
        let _: () = unsafe { msg_send![&*layer, removeAnimationForKey: &*key] };
    }
}

/// The view's layer faded in over `seconds`, in the sheet's curve.
fn fade_in(view: &AnyObject, seconds: f64) {
    let Some(layer) = layer_of(view) else {
        return;
    };
    let path = NSString::from_str("opacity");
    // SAFETY: CABasicAnimation answers an autoreleased animation of the key path, retained.
    let animation: Retained<AnyObject> =
        unsafe { msg_send![class!(CABasicAnimation), animationWithKeyPath: &*path] };
    // SAFETY: NSNumber holds a float.
    let clear: Retained<AnyObject> =
        unsafe { msg_send![class!(NSNumber), numberWithFloat: 0.0_f32] };
    // SAFETY: an animation takes the value it plays from, an object it retains; with no value
    // to play to, it plays to the layer's own.
    let _: () = unsafe { msg_send![&*animation, setFromValue: &*clear] };
    // SAFETY: how long it plays, a CFTimeInterval.
    let _: () = unsafe { msg_send![&*animation, setDuration: seconds] };
    // SAFETY: and its curve, which it retains.
    let _: () = unsafe { msg_send![&*animation, setTimingFunction: &*ease()] };
    add(&layer, &animation);
}

/// The sheet's `--ease`: cubic-bezier(.2, .8, .2, 1).
fn ease() -> Retained<AnyObject> {
    let points = (0.2_f32, 0.8_f32, 0.2_f32, 1.0_f32);
    // SAFETY: CAMediaTimingFunction answers an autoreleased curve of four float control points;
    // the pointer is retained before the pool lets it go.
    let curve: *mut AnyObject = unsafe {
        class!(CAMediaTimingFunction).send_message(sel!(functionWithControlPoints::::), points)
    };
    // SAFETY: the curve is a live object (or nil, answered as the linear curve's name below).
    unsafe { Retained::retain(curve) }.unwrap_or_else(|| {
        let linear = NSString::from_str("linear");
        // SAFETY: the named curve, which always exists.
        unsafe { msg_send![class!(CAMediaTimingFunction), functionWithName: &*linear] }
    })
}

/// For checks: where `view` goes and where it is drawn now, from its parent's top left, how
/// clear it is drawn, and how many of the animations added here it still plays.
pub(in crate::browser) fn drawn(view: Moved<'_>) -> Option<Drawn> {
    let view = view.view()?;
    let layer = layer_of(&view)?;
    let parent = parent(&view)?;
    // SAFETY: an NSView answers its bounds, a CGRect.
    let room: CGRect = unsafe { msg_send![&*parent, bounds] };
    let top = from_top(&view);
    let down = |[x, y, wide, tall]: Frame| match top {
        true => [x, y, wide, tall],
        false => [x, room.size.height - y - tall, wide, tall],
    };
    // SAFETY: a layer answers the copy of it drawn now, or nil before it was first drawn.
    let shown: Option<Retained<AnyObject>> = unsafe { msg_send![&*layer, presentationLayer] };
    let shown = shown.unwrap_or_else(|| layer.clone());
    // SAFETY: a layer answers its place in its parent, a CGPoint.
    let place: CGPoint = unsafe { msg_send![&*shown, position] };
    // SAFETY: its bounds, a CGRect.
    let bounds: CGRect = unsafe { msg_send![&*shown, bounds] };
    // SAFETY: where it is placed by, a CGPoint.
    let anchor: CGPoint = unsafe { msg_send![&*shown, anchorPoint] };
    // SAFETY: and its opacity, a float.
    let opacity: f32 = unsafe { msg_send![&*shown, opacity] };
    // SAFETY: whether what the layer holds is counted from its top, a BOOL.
    let held_from_top: bool = unsafe { msg_send![&*layer, contentsAreFlipped] };
    let now = down([
        place.x - anchor.x * bounds.size.width,
        place.y - anchor.y * bounds.size.height,
        bounds.size.width,
        bounds.size.height,
    ]);
    let stands = frame(&view);
    // What it holds is laid out for the view's own size, from the top of its bounds or from
    // their bottom: where its top comes on the screen follows from where the bounds begin.
    let top_held = match held_from_top {
        true => now[1] - bounds.origin.y,
        false => now[1] + now[3] - stands[3] + bounds.origin.y,
    };
    Some(Drawn {
        goes: down(stands),
        now,
        holds: (now[0] - bounds.origin.x, top_held),
        raised: -shift(&shown),
        opacity,
        playing: playing(&layer).len(),
        seen: seen(&view),
    })
}
