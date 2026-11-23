//! What the view draws and when: the terminal's frame when it says it changed (only the rows
//! that look new are drawn again into pictures, then the area is asked to draw, which lays the
//! pictures over its ground), the grid fitted to the area's size (the terminal and its program
//! told of a new one), the look from the person's settings and GTK's theme, and, while out of
//! sight, nothing.

use std::sync::Arc;

use gtk::cairo;
use gtk::prelude::{GtkSettingsExt, WidgetExt};

use super::super::TermSeen;
use super::super::painter::{Backend, Image, Native};
use super::state::State;
use super::view::View;
use super::{Picture, Placed, TermLook, TermPainter, acts, layout, overlay};
use crate::surface::Geometry;

/// Whether the person or GTK's theme asks for dark: the person's choice in Settings, else the
/// theme's (a dark one asked for, or one named for it).
fn dark() -> bool {
    use crate::browser::settings::Theme;
    match crate::browser::settings::now().theme {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => gtk::Settings::default().is_some_and(|settings| {
            settings.is_gtk_application_prefer_dark_theme()
                || (settings.gtk_theme_name())
                    .is_some_and(|name| name.to_lowercase().contains("dark"))
        }),
    }
}

/// Whether the person turned GTK's animations off: the cursor then stands still.
pub(super) fn less_motion() -> bool {
    gtk::Settings::default().is_some_and(|settings| !settings.is_gtk_enable_animations())
}

impl View {
    fn size(&self) -> (f64, f64) {
        let area = &self.area;
        (
            f64::from(area.allocated_width()),
            f64::from(area.allocated_height()),
        )
    }

    fn scale(&self) -> f64 {
        f64::from(self.area.scale_factor().max(1))
    }

    /// The look from the person's settings, GTK's theme and the screen's scale: a new one draws
    /// everything again, and the terminal is given its colours and Option (Alt) key.
    pub(crate) fn restyle(&self) {
        let settings = crate::browser::settings::now();
        let (scale, dark) = (self.scale(), dark());
        let Some(mut state) = self.state() else {
            return;
        };
        let at = acts::Where {
            dark,
            scale,
            accent: super::super::theme::ACCENT,
            active: state.active,
            key: state.key,
            display: state.display,
        };
        let look = acts::look(&settings, at);
        state.meta = acts::meta(settings.option_meta);
        state.surface.option_meta(state.meta);
        if (state.painter.as_ref()).is_some_and(|painter| *painter.look() == look) {
            return;
        }
        state.surface.colors(&look.palette());
        match state.painter.as_mut() {
            Some(painter) => painter.set_look(&look),
            None => state.painter = TermPainter::new(&look),
        }
        drop(state);
        self.fit();
        self.redraw(true);
    }

    /// The view drawn as the one in use (`active`: it has the keys in the active window) or not,
    /// its window the active one (`key`) or not.
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
        drop(state);
        self.redraw(true);
    }

    /// The grid that fits the area now, told to the terminal (and its program) when it or its
    /// cell's size in pixels is new (a font's size, a screen of another scale).
    pub(super) fn fit(&self) {
        let (width, height) = self.size();
        let Some(mut state) = self.state() else {
            return;
        };
        let Some(painter) = state.painter.as_ref() else {
            return;
        };
        let alternate = state.frame.alternate;
        let grid = painter.grid(width, height, alternate);
        let cell = painter.cell_pixels();
        state.fitted_alternate = alternate;
        let same = grid == state.grid && cell == state.cell;
        if width < 1.0 || height < 1.0 || same {
            return;
        }
        (state.grid, state.cell) = (grid, cell);
        state.resized += 1;
        let surface = Arc::clone(&state.surface);
        drop(state);
        surface.resize(grid.0, grid.1, cell);
        self.redraw(false);
    }

    /// The terminal's newest frame, as it said it changed.
    pub(crate) fn draw(&self) {
        self.redraw(false);
    }

    /// The newest frame (all of it when `full`) drawn into pictures, and over it the cursor, an
    /// input method's text, the line under a program that ended and the scrollbar; the area
    /// asked to draw them. Nothing while out of sight.
    pub(super) fn redraw(&self, full: bool) {
        let size = self.size();
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
        if state.surface.frame(&mut state.frame, full) {
            painter.paint(&state.frame);
            state.lines.keep(&state.frame);
        }
        let frame = &state.frame;
        let marked = (!state.marked.is_empty())
            .then_some(frame.cursor.at)
            .flatten()
            .and_then(|at| painter.marked(frame, &state.marked, at));
        let pointed = state.bar_pointed || state.bar_held.is_some();
        state.over.cursor = painter.cursor(frame).filter(|_| marked.is_none());
        state.over.marked = marked;
        state.over.ended = painter.ended(frame);
        state.over.bar = painter.scrollbar(frame, size, pointed);
        let blinks = overlay::blinks(frame, state.active, less_motion());
        let at = (state.over.cursor.as_ref())
            .map(|(_, at)| *at)
            .filter(|_| blinks);
        state.drawn += 1;
        let refit = state.frame.alternate != state.fitted_alternate;
        drop(guard);
        self.blink(at);
        self.area.queue_draw();
        if refit {
            self.fit();
        }
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
        if !shown {
            state.over = super::state::Over::default();
            if let Some(painter) = state.painter.as_mut() {
                painter.forget();
            }
            state.surface.hidden();
        }
        drop(state);
        self.area.set_visible(shown);
        if shown {
            self.fit();
            self.redraw(true);
        } else {
            self.blink(None);
        }
    }

    /// The view's geometry in pixels, as the terminal reads the pointer by it.
    pub(super) fn geometry(&self) -> Option<Geometry> {
        let (size, scale) = (self.size(), self.scale());
        let state = self.state()?;
        let cell = state.painter.as_ref()?.cell();
        Some(acts::geometry(size, scale, cell, state.frame.alternate))
    }

    /// The area's pictures laid over its ground, as GTK asks it to draw.
    pub(super) fn paint(&self, cr: &cairo::Context) {
        if let Some(state) = self.state() {
            compose(&state, cr);
        }
    }

    /// For checks: what the view drew last.
    pub(crate) fn seen(&self) -> Option<TermSeen> {
        let keys = self.has_keys();
        let state = self.state()?;
        let painter = state.painter.as_ref();
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
            layers: state
                .frame
                .rows
                .iter()
                .filter(|row| painter.is_some_and(|p| p.row(row.id).is_some()))
                .count(),
            marked: state.marked.clone(),
            drawn: state.drawn,
            alternate: state.frame.alternate,
            shape: state.frame.cursor.shape,
            selected: state.surface.copy(),
            history: state.frame.scroll.total,
            offset: state.frame.scroll.offset,
            bytes: painter.map_or(0, TermPainter::bytes),
            dark: painter.is_some_and(|painter| painter.look().dark),
            blinking: state.blink.is_some(),
            ended: state.frame.ended.clone(),
            // The system's secure input is macOS's alone.
            secure: false,
            resized: state.resized,
            // A drawing area paints its picture whole in its own bounds, and animates nothing.
            blank: 0,
            moving: 0,
            clips: true,
            lifted: 0,
            rect: [0; 4],
        })
    }

    /// For checks: the view as it draws itself, at the screen's scale.
    pub(crate) fn pixels(&self) -> Option<Picture> {
        let (size, scale) = (self.size(), self.scale());
        let state = self.state()?;
        let px = |points: f64| super::pixels(points * scale);
        let canvas = Native::canvas(px(size.0), px(size.1))?;
        let image = canvas.image()?;
        let cr = cairo::Context::new(&image).ok()?;
        cr.scale(scale, scale);
        compose(&state, &cr);
        drop(cr);
        Some(Picture {
            width: super::super::draw::Pen::width(&canvas),
            height: canvas.height(),
            rgba: canvas.rgba(),
        })
    }
}

/// The view's pictures laid over its ground in `cr` (points): its own colour where it is solid,
/// else nothing; each row at its place in view, the line under a program that ended, the cursor
/// in the shown half of its blinking, an input method's text, and the scrollbar while it shows.
fn compose(state: &State, cr: &cairo::Context) {
    let Some(painter) = state.painter.as_ref() else {
        return;
    };
    let look = painter.look();
    if look.solid {
        let ground = look.palette().background;
        let part = |c: u8| f64::from(c) / 255.0;
        cr.set_source_rgb(part(ground.r), part(ground.g), part(ground.b));
        let _ = cr.paint();
    }
    let pad = layout::gutter(state.frame.alternate);
    let tall = painter.cell().1;
    let scale = look.scale;
    for row in &state.frame.rows {
        if let Some((_, image)) = painter.row(row.id) {
            let top = f64::from(pad.top) + f64::from(row.y) * tall;
            put(cr, image, (0.0, top), scale);
        }
    }
    let over = &state.over;
    let cursor = over
        .cursor
        .as_ref()
        .filter(|_| state.blink.is_none() || state.blink_shown);
    let bar = over
        .bar
        .as_ref()
        .filter(|_| state.bar_shown || state.bar_held.is_some());
    for placed in [over.ended.as_ref(), cursor, over.marked.as_ref(), bar]
        .into_iter()
        .flatten()
    {
        let (image, [x, y, ..]): &Placed = placed;
        put(cr, image, (*x, *y), scale);
    }
}

/// `image` (device pixels) laid at `at` (points) of `cr`.
fn put(cr: &cairo::Context, image: &Image, at: (f64, f64), scale: f64) {
    let _ = cr.save();
    cr.translate(at.0, at.1);
    cr.scale(1.0 / scale, 1.0 / scale);
    if cr.set_source_surface(image, 0.0, 0.0).is_ok() {
        let _ = cr.paint();
    }
    let _ = cr.restore();
}
