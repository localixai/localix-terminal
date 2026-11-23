//! What the view draws and when: the terminal's frame when it says it changed (only the rows
//! that look new are drawn again, then every layer is moved or filled in one transaction), the
//! grid fitted to the view's size (the terminal and its program told of a new one), the look
//! from the person's settings, the window's appearance and the system's accent and display
//! options, and, while out of sight, nothing.

use std::sync::Arc;

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2_core_foundation::CGRect;
use objc2_core_graphics::CGContext;

use super::super::TermSeen;
use super::super::painter::Backend;
use super::canvas::Canvas;
use super::layers::still;
use super::view::TermView;
use super::{Picture, TermLook, TermPainter, acts, overlay};
use crate::surface::Geometry;

impl TermView {
    fn bounds(&self) -> CGRect {
        super::ns::bounds(self)
    }

    /// The screen's pixels to a point where the view is: its window's, else a Retina screen's.
    fn scale(&self) -> f64 {
        super::ns::scale(self)
    }

    /// Whether the view is drawn dark: its appearance, which follows the window's and the
    /// person's theme.
    fn dark(&self) -> bool {
        // SAFETY: an NSView answers its effective appearance.
        let appearance: Retained<AnyObject> = unsafe { msg_send![self, effectiveAppearance] };
        // SAFETY: an appearance answers its name, an NSString.
        let name: Retained<objc2_foundation::NSString> = unsafe { msg_send![&*appearance, name] };
        name.to_string().contains("Dark")
    }

    /// The look from the person's settings, the view's appearance and screen, the system's
    /// accent and its display options: a new one draws everything again, and the terminal is
    /// given its colours and Option key.
    pub(crate) fn restyle(&self) {
        let settings = crate::browser::settings::now();
        let (scale, dark) = (self.scale(), self.dark());
        let accent = super::ns::accent(self);
        let Some(mut state) = self.state() else {
            return;
        };
        let at = acts::Where {
            dark,
            scale,
            accent,
            active: state.active,
            key: state.key,
            display: state.display,
        };
        let look = acts::look(&settings, at);
        state.meta = acts::meta(settings.option_meta);
        state.surface.option_meta(state.meta);
        if state
            .painter
            .as_ref()
            .is_some_and(|painter| *painter.look() == look)
        {
            return;
        }
        state.surface.colors(&look.palette());
        let ground = look.solid.then(|| look.palette().background);
        state.layers.ground(ground);
        match state.painter.as_mut() {
            Some(painter) => painter.set_look(&look),
            None => state.painter = TermPainter::new(&look),
        }
        state.layers.rescale(scale);
        state.layers.clear();
        drop(state);
        self.fit();
        self.redraw(true);
    }

    /// The view drawn as the one in use (`active`: it has the keys in the key window) or not,
    /// its window the key one (`key`) or not: the cursor whole or an outline, the selection in
    /// the accent or grey, a find's matches in the accent or the system's grey.
    pub(super) fn activate(&self, active: bool, key: bool) {
        let Some(mut state) = self.state() else {
            return;
        };
        if (state.active, state.key) == (active, key) {
            return;
        }
        (state.active, state.key) = (active, key);
        let look = (state.painter.as_ref()).map(|painter| TermLook {
            active,
            key,
            ..painter.look().clone()
        });
        if let (Some(painter), Some(look)) = (state.painter.as_mut(), look) {
            painter.set_look(&look);
        }
        state.layers.clear();
        drop(state);
        self.redraw(true);
    }

    /// The grid that fits the view now, told to the terminal (and its program) when it or its
    /// cell's size in pixels is new (a font's size, a screen of another scale): not while the
    /// person moves its pane's edge (`hold.rs`), nor out of sight once it was in sight, where it
    /// keeps the grid it had (a pane put by for another's zoom comes back as it went, its
    /// program told nothing).
    pub(super) fn fit(&self) {
        let bounds = self.bounds();
        let Some(mut state) = self.state() else {
            return;
        };
        let held = state.divider || self.edge_held(state.edge);
        if held || (!state.shown && state.was_shown) {
            return;
        }
        let Some(painter) = state.painter.as_ref() else {
            return;
        };
        let alternate = state.frame.alternate;
        let grid = painter.grid(bounds.size.width, bounds.size.height, alternate);
        let cell = painter.cell_pixels();
        state.fitted_alternate = alternate;
        let same = grid == state.grid && cell == state.cell;
        if bounds.size.width < 1.0 || bounds.size.height < 1.0 || same {
            return;
        }
        (state.grid, state.cell) = (grid, cell);
        state.resized += 1;
        let surface = Arc::clone(&state.surface);
        drop(state);
        surface.resize(grid.0, grid.1, cell);
        self.redraw(false);
    }

    /// The terminal's newest frame, as it said it changed; and the system's secure input put
    /// right for what its program reads now, a password asked or done with no key pressed.
    pub(crate) fn draw(&self) {
        let asked = self.state().is_some_and(|state| state.active);
        super::events::secure(self, asked);
        self.redraw(false);
    }

    /// The newest frame (all of it when `full`), and over it the cursor, an input method's
    /// text and the scrollbar; nothing while out of sight.
    pub(super) fn redraw(&self, full: bool) {
        let bounds = self.bounds();
        let Some(mut guard) = self.state() else {
            return;
        };
        let state = &mut *guard;
        let Some(painter) = state.painter.as_mut() else {
            return;
        };
        if !state.shown {
            return;
        }
        let fresh = state.surface.frame(&mut state.frame, full);
        if fresh {
            painter.paint(&state.frame);
            state.lines.keep(&state.frame);
        }
        let frame = &state.frame;
        let marked = (!state.marked.is_empty())
            .then_some(frame.cursor.at)
            .flatten()
            .and_then(|at| painter.marked(frame, &state.marked, at));
        let layers = &mut state.layers;
        let size = (bounds.size.width, bounds.size.height);
        let bar = painter.scrollbar(frame, size, state.bar_pointed || state.bar_held.is_some());
        let ended = painter.ended(frame);
        let blinks = overlay::blinks(
            frame,
            state.active,
            crate::browser::appkit::reduced_motion(),
        );
        let mut missing = 0;
        still(|| {
            if fresh {
                missing = layers.rows(frame, painter);
            }
            layers.cursor(painter.cursor(frame).filter(|_| marked.is_none()), blinks);
            layers.marked(marked);
            layers.ended(ended);
            layers.scrollbar(bar);
        });
        state.drawn += 1;
        state.blank += u64::from(missing > 0);
        let refit = state.frame.alternate != state.fitted_alternate;
        drop(guard);
        if fresh {
            super::access::drew(self);
        }
        if refit {
            self.fit();
        }
        // While the grid is held, new output may move the cursor's row out of a shorter pane.
        self.lifted();
    }

    /// The view shown or out of sight: out of sight it lets go of every picture and of what the
    /// terminal drew from, and shown it draws a whole frame.
    pub(super) fn show(&self, shown: bool) {
        let Some(mut state) = self.state() else {
            return;
        };
        if state.shown == shown {
            return;
        }
        state.shown = shown;
        state.was_shown |= shown;
        if !shown {
            state.layers.clear();
            if let Some(painter) = state.painter.as_mut() {
                painter.forget();
            }
            state.surface.hidden();
        }
        drop(state);
        // SAFETY: an NSView takes whether it is hidden, a BOOL.
        let _: () = unsafe { msg_send![self, setHidden: Bool::new(!shown)] };
        if shown {
            self.fit();
            self.redraw(true);
        }
    }

    /// The view's geometry in pixels, as the terminal reads the pointer by it.
    pub(super) fn geometry(&self) -> Option<Geometry> {
        let bounds = self.bounds();
        let scale = self.scale();
        let state = self.state()?;
        let cell = state.painter.as_ref()?.cell();
        let size = (bounds.size.width, bounds.size.height);
        Some(acts::geometry(size, scale, cell, state.frame.alternate))
    }

    /// For checks: what the view drew last.
    pub(crate) fn seen(&self) -> Option<TermSeen> {
        let keys = self.has_keys();
        let state = self.state()?;
        Some(TermSeen {
            link: state.link.as_ref().map(|link| link.uri.clone()),
            found: state.frame.matches.len(),
            columns: state.frame.columns,
            lines: state.frame.lines,
            rows: state.lines.of(&state.frame),
            cursor: state.frame.cursor.at,
            keys,
            shown: state.shown,
            active: state.active,
            layers: state.layers.count(),
            marked: state.marked.clone(),
            drawn: state.drawn,
            alternate: state.frame.alternate,
            shape: state.frame.cursor.shape,
            selected: state.surface.copy(),
            history: state.frame.scroll.total,
            offset: state.frame.scroll.offset,
            bytes: state.painter.as_ref().map_or(0, TermPainter::bytes),
            dark: state
                .painter
                .as_ref()
                .is_some_and(|painter| painter.look().dark),
            blinking: state.layers.blinking(),
            ended: state.frame.ended.clone(),
            secure: state.secure,
            resized: state.resized,
            blank: state.blank,
            moving: state.layers.moving(),
            clips: state.layers.clips(),
            lifted: super::pixels(state.lift).try_into().unwrap_or(u32::MAX),
            rect: self.window_rect(),
        })
    }

    /// For checks: the view's layers as they are, drawn into a picture at the screen's scale.
    pub(crate) fn pixels(&self) -> Option<Picture> {
        let bounds = self.bounds();
        let scale = self.scale();
        let state = self.state()?;
        let px = |points: f64| super::pixels(points * scale);
        let canvas = Canvas::canvas(px(bounds.size.width), px(bounds.size.height))?;
        let context: &CGContext = canvas.context();
        // The canvas counts from the top already; the layers are drawn in points.
        CGContext::scale_ctm(Some(context), scale, scale);
        let context: *const CGContext = context;
        // SAFETY: a layer draws itself and its sublayers into a Core Graphics context, a
        // pointer to one.
        let _: () = unsafe {
            msg_send![&*state.layers.root, renderInContext: context.cast::<std::ffi::c_void>()]
        };
        Some(Picture {
            width: super::super::draw::Pen::width(&canvas),
            height: canvas.height(),
            rgba: canvas.rgba(),
        })
    }
}
