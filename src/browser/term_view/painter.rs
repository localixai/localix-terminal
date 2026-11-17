//! The terminal drawn, the same on every platform: a picture of a whole frame (what an agent is
//! shown of a terminal), and a painter that keeps each row drawn as an image for a view, drawing
//! again only the rows whose look changed, with the cursor, the scrollbar, an input method's text
//! and the line under a program that ended drawn apart. The platform gives the canvas, its fonts
//! and its images (`Backend`: Core Text into Core Graphics on macOS, Pango into Cairo on Linux).

use super::draw::{self, Faces, Grid, Pen};
use super::layout::{self, Deco, Line, RowPaint, Run};
use super::overlay::{self, SCROLLBAR_WIDTH};
use super::rows::{Cache, Painted};
use super::theme::{Paint, Theme};
use super::{Picture, TermLook};
use crate::surface::{Frame, RowId, Underline};

#[cfg(target_os = "linux")]
pub(crate) use super::linux::Canvas as Native;
#[cfg(target_os = "macos")]
pub(crate) use super::macos::Canvas as Native;

/// What a platform's canvas is besides its pen: made of a size, its fonts made of the settings'
/// stack, and turned into the image a view shows or the pixels a check reads.
pub(crate) trait Backend: Pen + Sized {
    type Image;
    /// The first font of `stack` that is installed, at `size` pixels, its ligatures joining or
    /// not; none when no font can be made.
    fn fonts(stack: &str, size: f64, ligatures: bool) -> Option<Self::Fonts>;
    /// A clear canvas of `width` by `height` pixels; none for an empty one.
    fn canvas(width: usize, height: usize) -> Option<Self>;
    fn image(&self) -> Option<Self::Image>;
    fn height(&self) -> usize;
    /// The pixels, a row after another, each red, green, blue and alpha, premultiplied.
    fn rgba(&self) -> Vec<u8>;
    /// The bytes an image takes.
    fn bytes(image: &Self::Image) -> usize;
}

/// An image of the platform's.
pub(crate) type Image = <Native as Backend>::Image;
type Fonts = <Native as Pen>::Fonts;

/// A picture and where it goes in a view: points across and down from its top left, wide and
/// tall.
pub(crate) type Placed = (Image, [f64; 4]);

/// The cells and the gutter of `frame` drawn in `fonts` at `scale`, in pixels.
fn grid(fonts: &Fonts, frame: &Frame, scale: f64) -> Grid {
    let pad = layout::gutter(frame.alternate);
    let cell = fonts.metrics().cell;
    Grid {
        cell: (cell.0.round().max(1.0), cell.1.round().max(1.0)),
        origin: (f64::from(pad.left) * scale, f64::from(pad.top) * scale),
        scale,
    }
}

/// The size in pixels of a view that holds `frame`'s grid and its gutter.
fn size(grid: &Grid, frame: &Frame) -> (usize, usize) {
    let pad = layout::gutter(frame.alternate);
    let across =
        f64::from(frame.columns) * grid.cell.0 + f64::from(pad.left + pad.right) * grid.scale;
    let down = f64::from(frame.lines) * grid.cell.1 + f64::from(pad.top + pad.bottom) * grid.scale;
    (pixels(across), pixels(down))
}

/// A length in pixels, rounded up, held to 0 and a screen's size.
pub(crate) fn pixels(n: f64) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a size in pixels, held to 0 and a screen's size just before"
    )]
    let n = n.ceil().clamp(0.0, 65_536.0) as usize;
    n
}

/// A picture of the whole of `frame` (every row, the cursor, the scrollbar and the line under a
/// program that ended) in `look`, on a clear ground: what the terminal shows, wherever its tab
/// is.
#[must_use]
pub fn term_picture(frame: &Frame, look: &TermLook) -> Option<Picture> {
    let canvas = drawn(frame, look)?;
    Some(Picture {
        width: canvas.width(),
        height: canvas.height(),
        rgba: canvas.rgba(),
    })
}

/// The canvas of the whole of `frame` drawn in `look`.
pub(crate) fn drawn(frame: &Frame, look: &TermLook) -> Option<Native> {
    let fonts = Native::fonts(&look.font, look.size * look.scale, look.ligatures)?;
    let theme = look.theme();
    let grid = grid(&fonts, frame, look.scale);
    let (width, height) = size(&grid, frame);
    let canvas = Native::canvas(width, height)?;
    for row in &frame.rows {
        let (paint, _) = layout::row(row, frame, theme);
        draw::row(&canvas, &fonts, &grid, &paint, row.y);
    }
    if let Some(cursor) = overlay::cursor(frame, theme) {
        draw::cursor(&canvas, &fonts, &grid, &cursor);
    }
    if let Some((y, paint)) = overlay::ended(frame, theme) {
        draw::row(&canvas, &fonts, &grid, &paint, y);
    }
    if let Some((top, length)) = overlay::scrollbar(frame, canvas.height() as f64 / grid.scale) {
        let s = grid.scale;
        let paint = theme.scrollbar(false);
        draw::scrollbar(&canvas, top * s, length * s, SCROLLBAR_WIDTH * s, paint);
    }
    Some(canvas)
}

/// The picture of a terminal for an agent: `shown` (the view's own frame, when the view is in
/// sight), else a whole frame asked of `surface` for the picture alone and let go of after it;
/// drawn in `painter`'s look, at a scale that keeps its long side within a screenshot's.
pub(crate) fn for_agent(
    painter: &TermPainter,
    shown: Option<&Frame>,
    surface: &dyn crate::surface::Surface,
) -> Option<Image> {
    let mut look = painter.look().clone();
    let mut asked = Frame::default();
    let frame = match shown {
        Some(frame) => frame,
        None => {
            let drew = surface.frame(&mut asked, true);
            surface.hidden();
            if !drew {
                return None;
            }
            &asked
        }
    };
    let (width, height) = painter.cell();
    let pad = layout::gutter(frame.alternate);
    let across = f64::from(frame.columns) * width + f64::from(pad.left + pad.right);
    let down = f64::from(frame.lines) * height + f64::from(pad.top + pad.bottom);
    let edge = crate::browser::capture::MAX_EDGE;
    look.scale = look.scale.min(edge / across.max(down).max(1.0));
    drawn(frame, &look)?.image()
}

/// Draws a view's rows, each into an image of its own, keeping what is drawn.
pub struct TermPainter {
    fonts: Fonts,
    look: TermLook,
    rows: Cache<Image>,
}

// SAFETY: the images are immutable once made, and the painter is used by one thread at a time.
#[cfg(target_os = "macos")]
unsafe impl Send for TermPainter {}

impl TermPainter {
    /// A painter in `look`; none when no font of its stack can be made.
    #[must_use]
    pub fn new(look: &TermLook) -> Option<Self> {
        Some(Self {
            fonts: Native::fonts(&look.font, look.size * look.scale, look.ligatures)?,
            look: look.clone(),
            rows: Cache::default(),
        })
    }

    /// Brings the rows' images up to `frame`: how many were drawn and how many kept.
    pub fn paint(&mut self, frame: &Frame) -> Painted {
        let theme = self.look.theme();
        let grid = grid(&self.fonts, frame, self.look.scale);
        let width = size(&grid, frame).0;
        let row_grid = Grid {
            origin: (grid.origin.0, 0.0),
            ..grid
        };
        let fonts = &self.fonts;
        let height = pixels(grid.cell.1);
        self.rows.keep(frame, theme, |_, paint| {
            let canvas = Native::canvas(width, height)?;
            draw::row(&canvas, fonts, &row_grid, paint, 0);
            canvas.image()
        })
    }

    /// The grid a view of `width` by `height` points holds in this painter's cells.
    #[must_use]
    pub fn grid(&self, width: f64, height: f64, alternate: bool) -> (u16, u16) {
        layout::grid(width, height, self.cell(), alternate)
    }

    /// The painter in `look` from now on: its fonts made again when the font, its size or the
    /// screen's scale changed; every picture let go of, since each was drawn in the old look.
    pub(crate) fn set_look(&mut self, look: &TermLook) {
        let same_fonts = self.look.font == look.font
            && self.look.ligatures == look.ligatures
            && (self.look.size - look.size).abs() < f64::EPSILON
            && (self.look.scale - look.scale).abs() < f64::EPSILON;
        let size = look.size * look.scale;
        if !same_fonts && let Some(fonts) = Native::fonts(&look.font, size, look.ligatures) {
            self.fonts = fonts;
        }
        look.clone_into(&mut self.look);
        self.rows.clear();
    }

    /// The picture kept of row `id` and the key of what it shows, while the row is in view.
    pub(crate) fn row(&self, id: RowId) -> Option<(u64, &Image)> {
        self.rows.picture(id)
    }

    /// The look the painter draws in.
    pub(crate) fn look(&self) -> &TermLook {
        &self.look
    }

    /// A cell's size, in points.
    pub(crate) fn cell(&self) -> (f64, f64) {
        let scale = self.look.scale;
        let cell = self.fonts.metrics().cell;
        (
            cell.0.round().max(1.0) / scale,
            cell.1.round().max(1.0) / scale,
        )
    }

    /// The cursor of `frame` drawn alone, and where it goes; none while it is hidden.
    pub(crate) fn cursor(&self, frame: &Frame) -> Option<Placed> {
        let paint = overlay::cursor(frame, self.look.theme())?;
        let grid = grid(&self.fonts, frame, self.look.scale);
        let (width, height) = grid.cell;
        let (left, top) = (f64::from(paint.column) * width, f64::from(paint.y) * height);
        let canvas = Native::canvas(pixels(f64::from(paint.width) * width), pixels(height))?;
        let alone = Grid {
            origin: (-left, -top),
            ..grid
        };
        draw::cursor(&canvas, &self.fonts, &alone, &paint);
        self.placed(&canvas, (grid.origin.0 + left, grid.origin.1 + top))
    }

    /// The scrollbar's slider of `frame` in a view `width` by `height` points, brighter when
    /// `pointed` at, and where it goes; none while there is no history.
    pub(crate) fn scrollbar(
        &self,
        frame: &Frame,
        (width, height): (f64, f64),
        pointed: bool,
    ) -> Option<Placed> {
        let (top, length) = overlay::scrollbar(frame, height)?;
        let s = self.look.scale;
        let canvas = Native::canvas(pixels(SCROLLBAR_WIDTH * s), pixels(length * s))?;
        let paint = self.look.theme().scrollbar(pointed);
        draw::scrollbar(&canvas, 0.0, length * s, SCROLLBAR_WIDTH * s, paint);
        self.placed(&canvas, ((width - SCROLLBAR_WIDTH) * s, top * s))
    }

    /// An input method's text as it composes, at column `column` of row `y` of `frame`: on the
    /// terminal's ground, underlined, so the cells under it do not show through.
    pub(crate) fn marked(
        &self,
        frame: &Frame,
        text: &str,
        (column, y): (u16, u16),
    ) -> Option<Placed> {
        let theme = self.look.theme();
        let (paint, across) = marked(text, theme);
        let grid = grid(&self.fonts, frame, self.look.scale);
        let (width, height) = grid.cell;
        let canvas = Native::canvas(pixels(f64::from(across.max(1)) * width), pixels(height))?;
        let alone = Grid {
            origin: (0.0, 0.0),
            ..grid
        };
        draw::row(&canvas, &self.fonts, &alone, &paint, 0);
        let at = (
            grid.origin.0 + f64::from(column) * width,
            grid.origin.1 + f64::from(y) * height,
        );
        self.placed(&canvas, at)
    }

    /// The line under a program that ended, drawn alone, and where it goes; none while it runs.
    pub(crate) fn ended(&self, frame: &Frame) -> Option<Placed> {
        let (y, paint) = overlay::ended(frame, self.look.theme())?;
        let grid = grid(&self.fonts, frame, self.look.scale);
        let canvas = Native::canvas(size(&grid, frame).0, pixels(grid.cell.1))?;
        let alone = Grid {
            origin: (grid.origin.0, 0.0),
            ..grid
        };
        draw::row(&canvas, &self.fonts, &alone, &paint, 0);
        self.placed(&canvas, (0.0, grid.origin.1 + f64::from(y) * grid.cell.1))
    }

    /// `canvas` as a picture put at `at` (pixels from the view's top left).
    fn placed(&self, canvas: &Native, at: (f64, f64)) -> Option<Placed> {
        let s = self.look.scale;
        let image = canvas.image()?;
        let (wide, tall) = (canvas.width() as f64 / s, canvas.height() as f64 / s);
        Some((image, [at.0 / s, at.1 / s, wide, tall]))
    }

    /// The bytes the kept images take.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.rows.pictures().map(Native::bytes).sum()
    }

    /// Lets go of every image (the view is out of sight): the next frame draws them all.
    pub fn forget(&mut self) {
        self.rows.clear();
    }
}

/// An input method's composing `text` laid out as a row of its own, on the terminal's ground
/// and underlined, and how many columns it takes.
fn marked(text: &str, theme: Theme) -> (RowPaint, u16) {
    let mut columns = Vec::new();
    let mut across = 0u16;
    for (at, c) in text.char_indices() {
        columns.push((u32::try_from(at).unwrap_or(u32::MAX), across));
        across = across.saturating_add(if wide(c) { 2 } else { 1 });
    }
    let ink = Paint::solid(theme.fg());
    let paint = RowPaint {
        fills: vec![layout::Fill {
            from: 0,
            to: across,
            paint: Paint::solid(theme.bg()),
        }],
        boxes: vec![],
        runs: vec![Run {
            bold: false,
            italic: false,
            paint: ink,
            text: text.to_string(),
            columns,
            end: across,
        }],
        decos: vec![Deco {
            from: 0,
            to: across,
            line: Line::Under(Underline::Single),
            paint: ink,
        }],
    };
    (paint, across)
}

/// Whether `c` takes two columns, as the scripts an input method composes in mostly do (Hangul,
/// the Han characters and kana, the full-width forms).
pub(crate) fn wide(c: char) -> bool {
    matches!(u32::from(c),
        0x1100..=0x115f | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe30..=0xfe4f
        | 0xff00..=0xff60 | 0xffe0..=0xffe6 | 0x1f300..=0x1f64f | 0x1f900..=0x1f9ff
        | 0x20000..=0x3fffd)
}
