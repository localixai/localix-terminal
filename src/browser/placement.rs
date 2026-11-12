//! Where the windows go on the screens: the main one in the middle of the first, or where the
//! person left it, kept in a file between launches, and each other one a step on from it. In
//! points, from the first screen's top left.

/// The part of a screen windows may use (the menu bar and the Dock left out).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A window's frame.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The smallest a window the person sees is made: its bar, and room for a page under it.
pub(super) const LEAST: tao::dpi::LogicalSize<f64> = tao::dpi::LogicalSize {
    width: 480.0,
    height: 192.0,
};

/// What the person moved or sized: where the window opens next, in this launch and the ones
/// after it (host.rs's `Shared`).
#[derive(Debug, Default, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Kept {
    pub deck: Option<Rect>,
    /// The tabs' column was left open or slid shut: a window opened again keeps it so.
    pub side: Option<bool>,
    /// The side's width as the person last dragged it.
    pub side_width: Option<f64>,
    /// The side shows the project's files under its tabs, or the person put them away.
    pub files: Option<bool>,
}

impl Kept {
    /// Read from `file`; nothing kept where it is missing or does not read.
    pub(super) fn read(file: &std::path::Path) -> Self {
        let mut kept: Self = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let whole = |rect: &Rect| {
            let all = [rect.x, rect.y, rect.width, rect.height];
            all.iter().all(|part| part.is_finite()) && rect.width > 0.0 && rect.height > 0.0
        };
        kept.deck = kept.deck.filter(whole);
        kept.side_width = kept.side_width.filter(|width| width.is_finite());
        kept
    }

    /// Written to `file`, whole: a new file moved over the old, so a crash leaves one or the
    /// other.
    pub(super) fn write(&self, file: &std::path::Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|why| why.to_string())?;
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
        }
        let new = file.with_extension("json.new");
        std::fs::write(&new, text + "\n").map_err(|why| format!("{}: {why}", new.display()))?;
        std::fs::rename(&new, file).map_err(|why| format!("{}: {why}", file.display()))
    }
}

/// The file `Kept` lies in between launches; none for windows the person does not see (a
/// test's), which keep nothing.
fn file() -> Option<std::path::PathBuf> {
    (!super::platform::unseen()).then(|| crate::paths::data().join("window.json"))
}

/// What the last launch left.
pub(super) fn recalled() -> Kept {
    file().map(|file| Kept::read(&file)).unwrap_or_default()
}

/// `kept` left for the next launch, written when it is not what the file already holds.
pub(super) fn keep(kept: Kept) {
    static WRITTEN: std::sync::Mutex<Option<Kept>> = std::sync::Mutex::new(None);
    let Some(file) = file() else { return };
    let mut written = crate::lock(&WRITTEN);
    if written.get_or_insert_with(|| Kept::read(&file)) == &kept {
        return;
    }
    match kept.write(&file) {
        Ok(()) => *written = Some(kept),
        Err(why) => log::warn!("the window's place is not kept: {why}"),
    }
}

/// The gap between a first window and the screen's edges, where it has no room to spare.
const MARGIN: f64 = 12.0;

/// The most a first window is given, on a screen with room to spare: a terminal with a page
/// beside it.
const MOST: tao::dpi::LogicalSize<f64> = tao::dpi::LogicalSize {
    width: 1200.0,
    height: 760.0,
};

/// The part of a screen a first window takes at most, each way: the desktop and the apps
/// around it stay in sight, as a window the person would size themselves leaves them.
const SHARE: (f64, f64) = (0.76, 0.8);

/// What a first window is given where the screen has it, however small that share comes out.
const ROOMY: tao::dpi::LogicalSize<f64> = tao::dpi::LogicalSize {
    width: 960.0,
    height: 600.0,
};

/// A first window's length one way on a screen with `room` that way.
fn first(most: f64, share: f64, roomy: f64, room: f64) -> f64 {
    let fits = (room - 2.0 * MARGIN).max(0.0);
    most.min((room * share).round()).max(roomy).min(fits)
}

/// How much of a kept frame must lie on a screen for the window to be in reach there: its bar
/// to take hold of, and a part of what it shows.
const REACH: tao::dpi::LogicalSize<f64> = tao::dpi::LogicalSize {
    width: 160.0,
    height: 80.0,
};

/// How much of `rect` lies on `area`, each way.
fn shared(area: Area, rect: Rect) -> (f64, f64) {
    let along = |from: f64, len: f64, start: f64, room: f64| {
        ((from + len).min(start + room) - from.max(start)).max(0.0)
    };
    (
        along(rect.x, rect.width, area.x, area.width),
        along(rect.y, rect.height, area.y, area.height),
    )
}

/// The screen of `areas` most of `rect` lies on; the first where it lies on none.
pub(super) fn home(areas: &[Area], rect: Rect) -> Area {
    let on = |area: &Area| {
        let (width, height) = shared(*area, rect);
        width * height
    };
    let most = (areas.iter()).max_by(|a, b| on(a).total_cmp(&on(b)));
    match (most, areas.first()) {
        (Some(most), _) if on(most) > 0.0 => *most,
        (_, Some(first)) => *first,
        _ => Area {
            x: 0.0,
            y: 0.0,
            width: MOST.width,
            height: MOST.height,
        },
    }
}

/// The main window on `areas`, the first of them the screen a first window opens on.
///
/// `kept`, where the person set one and enough of it is still on a screen: as they left it, no
/// larger than that screen (a smaller one may have taken its place). Else in the middle of the
/// first screen, a third of the room left above it as the system centres a window, at a part
/// of the screen that leaves what is around it in sight.
pub(super) fn deck(areas: &[Area], kept: Option<Rect>) -> Rect {
    if let Some(kept) = kept {
        let area = home(areas, kept);
        let (width, height) = shared(area, kept);
        if width >= REACH.width.min(kept.width) && height >= REACH.height.min(kept.height) {
            let size = (
                kept.width.min(area.width).max(LEAST.width),
                kept.height.min(area.height).max(LEAST.height),
            );
            let fits = kept.width <= area.width && kept.height <= area.height;
            return Rect {
                x: if fits { kept.x } else { area.x },
                y: if fits { kept.y } else { area.y },
                width: size.0,
                height: size.1,
            };
        }
    }
    let area = home(
        areas,
        Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
    );
    let width = first(MOST.width, SHARE.0, ROOMY.width, area.width);
    let height = first(MOST.height, SHARE.1, ROOMY.height, area.height);
    Rect {
        x: area.x + ((area.width - width) / 2.0).round(),
        y: area.y + ((area.height - height) / 3.0).round().max(MARGIN),
        width,
        height,
    }
}

/// The place of the window `steps` steps after the main one (the first other window is 1):
/// `deck` moved down and right by a step each, as the system cascades new windows, so two never
/// lie on one another. Where the next step would leave `area`, it starts again from the area's
/// top left.
pub(super) fn cascade(area: Area, deck: Rect, steps: usize) -> Rect {
    const STEP: f64 = 28.0;
    let steps = f64::from(u32::try_from(steps).unwrap_or(u32::MAX));
    let along = |from: f64, start: f64, room: f64| {
        let room = room.max(0.0);
        let moved = from - start + STEP * steps;
        start + moved.rem_euclid(room + STEP).min(room)
    };
    Rect {
        x: along(deck.x, area.x, area.width - deck.width),
        y: along(deck.y, area.y, area.height - deck.height),
        ..deck
    }
}

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
