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

/// Where a page of `frame` goes in the pages' part of the window (`size`, `top` points down): its
/// offset across, its top and its size; with no frame, the whole part. A frame larger than the
/// part is cut to it.
pub(crate) fn place(
    size: LogicalSize<f64>,
    top: f64,
    frame: Option<&Frame>,
) -> (f64, f64, LogicalSize<f64>) {
    let Some(frame) = frame else {
        return (0.0, top, size);
    };
    let width = frame.width.min(size.width);
    let height = frame.height.min(size.height);
    let across = ((size.width - width) / 2.0).floor();
    let down = ((size.height - height) / 2.0).floor();
    (across, top + down, LogicalSize::new(width, height))
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

/// The strip's call that shows the size of the tab shown (`F(null)` for the whole window).
pub(crate) fn script(frame: Option<&Frame>) -> String {
    let data = frame.map(|frame| {
        serde_json::json!({"name": frame.name, "width": frame.width.round(), "height": frame.height.round()})
    });
    format!("F({})", serde_json::json!(data))
}

#[cfg(test)]
#[path = "sizes_tests.rs"]
mod tests;
