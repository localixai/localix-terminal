//! The view's layers: a root over the window's glass, clear (nothing paints the terminal's
//! ground, but under Reduce Transparency, where it is the terminal's own), its rows counted
//! from the top; a layer for each row in view, by the core's id of the
//! row, holding the picture kept of what the row shows (a picture is set again only when the row
//! shows something else); and over the rows the cursor, an input method's text and the
//! scrollbar. Every change is made in one transaction with no animation.

use std::collections::BTreeMap;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGImage;
use objc2_foundation::{NSArray, NSString};

use super::{Placed, TermPainter, layout, overlay};
use crate::surface::{Frame, RowId};

/// The key of the cursor's blinking among its layer's animations.
const BLINK: &str = "blink";

/// A layer of no contents, at no place yet, scaled for the screen.
fn layer(scale: f64) -> Retained<AnyObject> {
    // SAFETY: `new` on CALayer answers a retained, empty layer.
    let made: Retained<AnyObject> = unsafe { msg_send![class!(CALayer), new] };
    // SAFETY: a layer takes its contents' scale, a CGFloat.
    let _: () = unsafe { msg_send![&*made, setContentsScale: scale] };
    // SAFETY: and whether it is opaque, a BOOL.
    let _: () = unsafe { msg_send![&*made, setOpaque: Bool::NO] };
    made
}

fn rect([x, y, width, height]: [f64; 4]) -> CGRect {
    CGRect {
        origin: CGPoint { x, y },
        size: CGSize { width, height },
    }
}

fn frame_of(layer: &AnyObject, at: [f64; 4]) {
    // SAFETY: a layer takes its frame, a CGRect in its superlayer's points.
    let _: () = unsafe { msg_send![layer, setFrame: rect(at)] };
}

fn contents(layer: &AnyObject, image: Option<&CGImage>) {
    let image: *const AnyObject = image.map_or(std::ptr::null(), |image| {
        let image: *const CGImage = image;
        image.cast()
    });
    // SAFETY: a layer's contents are an object; a CGImage is one (toll-free), or nil for none.
    let _: () = unsafe { msg_send![layer, setContents: image] };
}

fn hidden(layer: &AnyObject, hidden: bool) {
    // SAFETY: a layer takes whether it is hidden, a BOOL.
    let _: () = unsafe { msg_send![layer, setHidden: Bool::new(hidden)] };
}

/// Changes made to layers within, shown together and never animated.
pub(super) fn still(change: impl FnOnce()) {
    // SAFETY: a transaction begun is committed below, on the same (main) thread.
    let _: () = unsafe { msg_send![class!(CATransaction), begin] };
    // SAFETY: the transaction runs no animation.
    let _: () = unsafe { msg_send![class!(CATransaction), setDisableActions: Bool::YES] };
    change();
    // SAFETY: as above.
    let _: () = unsafe { msg_send![class!(CATransaction), commit] };
}

pub(super) struct Layers {
    pub(super) root: Retained<AnyObject>,
    /// Each row's layer, by its id, and the key of the picture it holds.
    rows: BTreeMap<RowId, (Retained<AnyObject>, u64)>,
    cursor: Retained<AnyObject>,
    marked: Retained<AnyObject>,
    ended: Retained<AnyObject>,
    bar: Retained<AnyObject>,
    scale: f64,
    /// Where the cursor was when it began to blink: it blinks anew from shown once it moves.
    blinking: Option<[f64; 4]>,
}

impl Layers {
    pub(super) fn new(scale: f64) -> Self {
        let root = layer(scale);
        // SAFETY: a layer takes whether its sublayers count from the top (a BOOL); it adds each
        // overlay over the rows, which go in under them.
        unsafe {
            let _: () = msg_send![&*root, setGeometryFlipped: Bool::YES];
        }

        let (cursor, marked, bar) = (layer(scale), layer(scale), layer(scale));
        let ended = layer(scale);
        // The scrollbar shows only while the pointer is over the view (`bar_shown`).
        // SAFETY: a layer takes its opacity, a float.
        let _: () = unsafe { msg_send![&*bar, setOpacity: 0.0_f32] };
        for over in [&cursor, &marked, &ended, &bar] {
            hidden(over, true);
            // SAFETY: as above.
            let _: () = unsafe { msg_send![&*root, addSublayer: &**over] };
        }
        Self {
            root,
            rows: BTreeMap::new(),
            cursor,
            marked,
            ended,
            bar,
            scale,
            blinking: None,
        }
    }

    /// The screen's scale changed: every layer's pictures are of the new one.
    pub(super) fn rescale(&mut self, scale: f64) {
        self.scale = scale;
        for layer in (self.rows.values().map(|(layer, _)| layer)).chain([
            &self.cursor,
            &self.marked,
            &self.ended,
            &self.bar,
        ]) {
            // SAFETY: a layer takes its contents' scale, a CGFloat.
            let _: () = unsafe { msg_send![&**layer, setContentsScale: scale] };
        }
    }

    /// The rows of `frame` as `painter` keeps them drawn, each at its place in view; the layers
    /// of rows gone from view let go of. How many rows of the frame have no picture.
    pub(super) fn rows(&mut self, frame: &Frame, painter: &TermPainter) -> usize {
        let mut missing = 0;
        let pad = layout::gutter(frame.alternate);
        let tall = painter.cell().1;
        for row in &frame.rows {
            let Some((key, image)) = painter.row(row.id) else {
                missing += 1;
                continue;
            };
            let image: &CGImage = image;
            let (wide, high) = (
                CGImage::width(Some(image)) as f64 / self.scale,
                CGImage::height(Some(image)) as f64 / self.scale,
            );
            let at = [
                0.0,
                f64::from(pad.top) + f64::from(row.y) * tall,
                wide,
                high,
            ];
            let entry = self.rows.entry(row.id).or_insert_with(|| {
                let made = layer(self.scale);
                // SAFETY: a layer inserts a sublayer at an index of its sublayers: under the
                // overlays, which were added first and stay over it.
                let _: () =
                    unsafe { msg_send![&*self.root, insertSublayer: &*made, atIndex: 0_u32] };
                (made, u64::MAX)
            });
            if entry.1 != key {
                contents(&entry.0, Some(image));
                entry.1 = key;
            }
            frame_of(&entry.0, at);
        }
        self.rows.retain(|id, (layer, _)| {
            let kept = frame.rows.iter().any(|row| row.id == *id);
            if !kept {
                // SAFETY: a layer leaves its superlayer.
                let _: () = unsafe { msg_send![&**layer, removeFromSuperlayer] };
            }
            kept
        });
        missing
    }

    /// The cursor, blinking or not (`blinks`): shown for the first half of each period and
    /// hidden for the second, from shown again where it moved, as the terminal page's did.
    pub(super) fn cursor(&mut self, placed: Option<Placed>, blinks: bool) {
        let at = placed.as_ref().map(|(_, at)| *at).filter(|_| blinks);
        if at != self.blinking {
            let key = NSString::from_str(BLINK);
            // SAFETY: a layer lets go of its animation by a key, there or not.
            let _: () = unsafe { msg_send![&*self.cursor, removeAnimationForKey: &*key] };
            if at.is_some() {
                blink(&self.cursor);
            }
            self.blinking = at;
        }
        put(&self.cursor, placed);
    }

    /// Whether the cursor blinks, for checks.
    pub(super) fn blinking(&self) -> bool {
        self.blinking.is_some()
    }

    pub(super) fn ended(&self, placed: Option<Placed>) {
        put(&self.ended, placed);
    }

    pub(super) fn marked(&self, placed: Option<Placed>) {
        put(&self.marked, placed);
    }

    pub(super) fn scrollbar(&self, placed: Option<Placed>) {
        put(&self.bar, placed);
    }

    /// The scrollbar shown or not, coming or going over `fade` seconds (none: at once).
    pub(super) fn bar_shown(&self, shown: bool, fade: f64) {
        let opacity: f32 = if shown { 1.0 } else { 0.0 };
        // SAFETY: a transaction begun is committed below, on the same (main) thread; its
        // duration is the fade's, and the layer's opacity animates as the system animates it.
        unsafe {
            let _: () = msg_send![class!(CATransaction), begin];
        }
        // SAFETY: as above.
        let _: () = unsafe { msg_send![class!(CATransaction), setAnimationDuration: fade] };
        // SAFETY: a layer takes its opacity, a float.
        let _: () = unsafe { msg_send![&*self.bar, setOpacity: opacity] };
        // SAFETY: as above.
        let _: () = unsafe { msg_send![class!(CATransaction), commit] };
    }

    /// The root's ground: the terminal's own colour, or clear (`None`) over the glass.
    pub(super) fn ground(&self, colour: Option<crate::surface::Rgb>) {
        let part = |c: u8| f64::from(c) / 255.0;
        let made: Option<Retained<AnyObject>> = colour.map(|c| {
            // SAFETY: NSColor's class method takes four CGFloats and returns a colour.
            unsafe {
                msg_send![class!(NSColor), colorWithSRGBRed: part(c.r), green: part(c.g),
                    blue: part(c.b), alpha: 1.0_f64]
            }
        });
        let cg: *const std::ffi::c_void = made.as_ref().map_or(std::ptr::null(), |colour| {
            // SAFETY: an NSColor answers its CGColor, alive as long as the colour.
            unsafe { msg_send![&**colour, CGColor] }
        });
        // SAFETY: a layer takes its background colour, a CGColor or NULL for none; it retains it.
        let _: () = unsafe { msg_send![&*self.root, setBackgroundColor: cg] };
    }

    /// Every picture let go of (the view is out of sight, or draws everything again), with no
    /// animation, as every other change.
    pub(super) fn clear(&mut self) {
        still(|| {
            for (_, (layer, _)) in std::mem::take(&mut self.rows) {
                // SAFETY: a layer leaves its superlayer.
                let _: () = unsafe { msg_send![&*layer, removeFromSuperlayer] };
            }
            self.cursor(None, false);
            for over in [&self.marked, &self.ended, &self.bar] {
                put(over, None);
            }
        });
    }

    /// How many rows have a layer, for checks.
    pub(super) fn count(&self) -> usize {
        self.rows.len()
    }

    /// How many layers an animation moves now, the cursor's blinking left out, for checks.
    pub(super) fn moving(&self) -> usize {
        let blink = NSString::from_str(BLINK);
        let rows = self.rows.values().map(|(layer, _)| layer);
        let all = rows.chain([&self.root, &self.cursor, &self.marked, &self.ended]);
        all.filter(|layer| {
            // SAFETY: a layer answers the keys of its animations, an array of strings or nil.
            let keys: Option<Retained<NSArray<NSString>>> =
                unsafe { msg_send![&***layer, animationKeys] };
            keys.is_some_and(|keys| keys.iter().any(|key| *key != *blink))
        })
        .count()
    }

    /// Whether nothing of the layers reaches past the root's bounds, for checks.
    pub(super) fn clips(&self) -> bool {
        // SAFETY: a layer answers whether it masks its sublayers to its bounds, a BOOL.
        let masks: Bool = unsafe { msg_send![&*self.root, masksToBounds] };
        masks.as_bool()
    }
}

/// `layer` blinking from now on: its opacity full for the first half of each period and none
/// for the second, stepped, over and over, run by the system's animation server.
fn blink(layer: &AnyObject) {
    let path = NSString::from_str("opacity");
    // SAFETY: CAKeyframeAnimation's class method makes an animation of a key path, autoreleased.
    let animation: Retained<AnyObject> =
        unsafe { msg_send![class!(CAKeyframeAnimation), animationWithKeyPath: &*path] };
    let number = |value: f64| -> Retained<AnyObject> {
        // SAFETY: NSNumber's class method answers a number of a double.
        unsafe { msg_send![class!(NSNumber), numberWithDouble: value] }
    };
    let values = NSArray::from_retained_slice(&[number(1.0), number(0.0)]);
    let times = NSArray::from_retained_slice(&[number(0.0), number(0.5), number(1.0)]);
    let discrete = NSString::from_str("discrete");
    // SAFETY: the animation takes its values and their times, arrays of numbers (a discrete
    // animation has one time more than values), and how it moves between them, a string.
    unsafe {
        let _: () = msg_send![&*animation, setValues: &*values];
    }
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*animation, setKeyTimes: &*times] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*animation, setCalculationMode: &*discrete] };
    // SAFETY: its period, a CFTimeInterval, and how often it repeats, a float: always.
    let _: () = unsafe { msg_send![&*animation, setDuration: overlay::BLINK] };
    // SAFETY: as above.
    let _: () = unsafe { msg_send![&*animation, setRepeatCount: f32::INFINITY] };
    let key = NSString::from_str(BLINK);
    // SAFETY: a layer runs an animation by a key, copying it.
    let _: () = unsafe { msg_send![layer, addAnimation: &*animation, forKey: &*key] };
}

/// `layer` showing `placed`, or hidden and empty for none.
fn put(layer: &AnyObject, placed: Option<Placed>) {
    match placed {
        Some((image, at)) => {
            contents(layer, Some(&*image));
            frame_of(layer, at);
            hidden(layer, false);
        }
        None => {
            contents(layer, None);
            hidden(layer, true);
        }
    }
}
