//! The page's size: a device's viewport, framed in the middle of the window, or the whole window.
//! The person picks one from the strip's menu, the agent names one (`size`); each tab keeps its
//! own. Layout only: the user agent, the pixel ratio and touch stay the window's.

use tao::dpi::LogicalSize;

/// A size set on a tab: what it is called and its CSS viewport.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Frame {
    pub(crate) name: String,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

/// The sizes offered, small to large: the CSS viewports of the devices they are named after.
pub(crate) const PRESETS: [(&str, f64, f64); 6] = [
    ("iPhone SE", 375.0, 667.0),
    ("iPhone 15 Pro", 393.0, 852.0),
    ("Pixel 8", 412.0, 915.0),
    ("iPad Mini", 768.0, 1024.0),
    ("Laptop", 1280.0, 800.0),
    ("Desktop", 1440.0, 900.0),
];

/// Short words an agent may use for a preset.
const ALIASES: [(&str, &str); 3] = [
    ("mobile", "iPhone 15 Pro"),
    ("phone", "iPhone 15 Pro"),
    ("tablet", "iPad Mini"),
];

/// The words for no frame: the page fills the window.
const WHOLE: [&str; 4] = ["window", "fill", "responsive", "none"];

/// The smallest and largest side a size may have, in points.
const SIDES: (f64, f64) = (200.0, 4000.0);

impl Frame {
    fn preset((name, width, height): (&str, f64, f64)) -> Self {
        Self {
            name: name.into(),
            width,
            height,
        }
    }

    /// The same frame turned a quarter: its width and height swapped.
    pub(crate) fn rotated(&self) -> Self {
        Self {
            name: self.name.clone(),
            width: self.height,
            height: self.width,
        }
    }

    /// `393 × 852`.
    pub(crate) fn sides(&self) -> String {
        format!("{} × {}", self.width.round(), self.height.round())
    }
}

/// The frame `value` names: a preset (`iPhone 15 Pro`, any case), `mobile` or `tablet`,
/// `393x852`; `window` for none. Why not, when it names none.
pub(crate) fn parse(value: &str) -> Result<Option<Frame>, String> {
    let words = |text: &str| text.to_lowercase().replace(' ', "");
    let wanted = words(value.trim());
    if WHOLE.contains(&wanted.as_str()) {
        return Ok(None);
    }
    let alias = ALIASES.iter().find(|(word, _)| *word == wanted);
    let wanted = alias.map_or(wanted, |(_, name)| words(name));
    if let Some(&preset) = PRESETS.iter().find(|(name, ..)| words(name) == wanted) {
        return Ok(Some(Frame::preset(preset)));
    }
    let sides = wanted.split_once(['x', '×', '*']).and_then(|(w, h)| {
        let side = |text: &str| {
            text.parse::<f64>()
                .ok()
                .filter(|n| (SIDES.0..=SIDES.1).contains(n))
        };
        Some((side(w)?, side(h)?))
    });
    match sides {
        Some((width, height)) => Ok(Some(Frame {
            name: "Custom".into(),
            width,
            height,
        })),
        None => {
            let names: Vec<&str> = PRESETS.iter().map(|(name, ..)| *name).collect();
            Err(format!(
                "no page size `{value}`: it takes window, mobile, tablet, {}, or WIDTHxHEIGHT ({}–{} points)",
                names.join(", "),
                SIDES.0,
                SIDES.1
            ))
        }
    }
}

/// Where a page goes in the pages' part of the window, and at what scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placed {
    /// Its offset across the part, in points.
    pub(crate) across: f64,
    /// Its top, in points from the window's.
    pub(crate) down: f64,
    /// The points it takes on the screen.
    pub(crate) size: LogicalSize<f64>,
    /// Points per CSS pixel: below 1 when the frame is larger than the part.
    pub(crate) scale: f64,
}

/// Where a page of `frame` goes in the pages' part of the window (`size`, `top` points down);
/// with no frame, the whole part. A frame larger than the part is drawn smaller to fit it, as a
/// browser's device view does: the page still lays out at the frame's size, and the window is
/// never made larger for it.
pub(crate) fn place(size: LogicalSize<f64>, top: f64, frame: Option<&Frame>) -> Placed {
    let Some(frame) = frame else {
        return Placed {
            across: 0.0,
            down: top,
            size,
            scale: 1.0,
        };
    };
    let sides = (frame.width.round(), frame.height.round());
    let fits = (size.width / sides.0).min(size.height / sides.1);
    let (width, height, scale) = if fits.is_finite() && fits > 0.0 && fits < 1.0 {
        smaller(size, sides, fits)
    } else {
        (sides.0, sides.1, 1.0)
    };
    Placed {
        across: ((size.width - width) / 2.0).floor().max(0.0),
        down: top + ((size.height - height) / 2.0).floor().max(0.0),
        size: LogicalSize::new(width, height),
        scale,
    }
}

/// How far under the exact scale a smaller page is drawn: WebKit keeps the scale with less
/// precision than this and cuts the page's width and height down to whole CSS pixels, so at the
/// exact scale a side could come out one under the frame's.
const HAIR: f64 = 1e-5;

/// The whole points and the scale of a frame `sides` drawn smaller (`fits` of it) in the pages'
/// part `room`. The system takes a view's size in whole points (what it is given is rounded), and
/// the page lays out at those points over the scale: so the points are chosen whole, and the
/// scale from them, such that both sides lay out at the frame's. The sides tried go down a point
/// at a time from the first whole points at or over the exact ones (never past the part); the
/// one with the largest scale is taken, and with none exact, the first, which keeps both sides
/// at least the frame's.
fn smaller(room: LogicalSize<f64>, sides: (f64, f64), fits: f64) -> (f64, f64, f64) {
    const TRIED: u8 = 8;
    let most = |side: f64, room: f64| {
        (side * fits - 1e-9)
            .ceil()
            .clamp(1.0, room.floor().max(1.0))
    };
    let (wide, tall) = (most(sides.0, room.width), most(sides.1, room.height));
    let scaled = |w: f64, h: f64| ((w / sides.0).min(h / sides.1) * (1.0 - HAIR), w, h);
    let exact =
        |(scale, w, h): &(f64, f64, f64)| w / scale < sides.0 + 1.0 && h / scale < sides.1 + 1.0;
    let tried = (0..TRIED).flat_map(|less| {
        (0..TRIED).map(move |fewer| (wide - f64::from(less), tall - f64::from(fewer)))
    });
    let found = (tried.filter(|&(w, h)| w >= 1.0 && h >= 1.0))
        .map(|(w, h)| scaled(w, h))
        .filter(exact)
        .max_by(|a, b| a.0.total_cmp(&b.0));
    let (scale, w, h) = found.unwrap_or_else(|| scaled(wide, tall));
    (w, h, scale)
}

/// The strip's size menu: the whole window, each preset with its sides, and Rotate while a size
/// is set; with the item of the size set now.
pub(crate) fn menu(now: Option<&Frame>) -> (Vec<String>, Option<usize>) {
    let mut items = vec!["Fit to Window".to_string()];
    let mut marked = now.is_none().then_some(0);
    for (at, preset) in PRESETS.iter().enumerate() {
        let frame = Frame::preset(*preset);
        if now.is_some_and(|now| now.name == frame.name) {
            marked = Some(at + 1);
        }
        items.push(format!("{}  {}", frame.name, frame.sides()));
    }
    if let Some(now) = now {
        if now.name == "Custom" {
            items.push(format!("Custom  {}", now.sides()));
            marked = Some(items.len() - 1);
        }
        items.push("Rotate".into());
    }
    (items, marked)
}

/// What the person picked from the size menu.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Picked {
    /// The page fills the window.
    Window,
    Size(Frame),
}

impl Picked {
    /// The tab's frame it gives: none for the whole window.
    pub(crate) fn frame(self) -> Option<Frame> {
        match self {
            Self::Window => None,
            Self::Size(frame) => Some(frame),
        }
    }
}

/// What the menu's item `at` gives, from the size set `now`; none for an item it does not have.
pub(crate) fn picked(at: usize, now: Option<&Frame>) -> Option<Picked> {
    let (items, _) = menu(now);
    let item = items.get(at)?;
    let size = |frame: Option<Frame>| frame.map_or(Picked::Window, Picked::Size);
    Some(match at {
        0 => Picked::Window,
        _ if item == "Rotate" => size(now.map(Frame::rotated)),
        _ if at <= PRESETS.len() => {
            let frame = Frame::preset(PRESETS[at - 1]);
            // The preset set now, turned: picked again it stays as turned.
            let turned = now.filter(|now| now.name == frame.name && now.width == frame.height);
            Picked::Size(turned.cloned().unwrap_or(frame))
        }
        _ => size(now.cloned()),
    })
}

/// The strip's call that shows the size of the tab shown and the scale it is drawn at
/// (`F(null)` for the whole window).
pub(crate) fn script(frame: Option<&Frame>, scale: f64) -> String {
    let data = frame.map(|frame| {
        let percent = (scale * 100.0).round();
        serde_json::json!({"name": frame.name, "width": frame.width.round(), "height": frame.height.round(), "percent": percent})
    });
    format!("F({})", serde_json::json!(data))
}

#[cfg(test)]
#[path = "sizes_tests.rs"]
mod tests;
