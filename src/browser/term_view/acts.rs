//! What the person's acts in a terminal's view mean, the same on every platform: the view's own
//! Command (Super) keys, the text a key typed, the wheel's turn in whole rows, the menu a right
//! click opens and what its items do, the cell under the pointer, the scrollbar's slider taken
//! and dragged, the cursor's cell for an input method's candidates, and the look and geometry a
//! view is drawn and read in. Each platform reads its events into these and carries them out.

use super::TermLook;
use super::layout;
use super::overlay::SCROLLBAR_WIDTH;
use crate::browser::choice::Entry;
use crate::surface::{Frame, Geometry, Key, KeyCode, Mods, OptionMeta, Scrollbar};

/// What a key held with Command (Super) does in the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    /// A terminal's own (⌘←, ⌘→, ⌘⌫, ⌘⌦): told to the terminal as the key it is.
    Send,
    /// ⌘K: history and screen cleared.
    Clear,
    /// ⌘↑ and ⌘↓: to the top of the history and to the bottom.
    Top,
    Bottom,
    /// Anything else: the window's menu's, or nothing.
    Nothing,
}

/// What `key`, held with Command, does in the view.
pub(crate) fn command(key: &Key) -> Command {
    const MAC: [KeyCode; 4] = [
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::Backspace,
        KeyCode::Delete,
    ];
    if MAC.contains(&key.code) {
        return Command::Send;
    }
    if key.mods.has(Mods::ALT) || key.mods.has(Mods::SHIFT) {
        return Command::Nothing;
    }
    if key.code == KeyCode::Letter(b'k') {
        Command::Clear
    } else if key.code == KeyCode::ArrowUp {
        Command::Top
    } else if key.code == KeyCode::ArrowDown {
        Command::Bottom
    } else {
        Command::Nothing
    }
}

/// What a key typed, when it typed text and not a control character.
pub(crate) fn printable(text: &str) -> String {
    match text.chars().any(char::is_control) {
        true => String::new(),
        false => text.to_string(),
    }
}

/// The modifiers the layout used to make `text`: none for no text, else those of `mods` among
/// `used`.
pub(crate) const fn consumed(text: &str, mods: Mods, used: u16) -> Mods {
    Mods(if text.is_empty() { 0 } else { mods.0 & used })
}

/// The wheel turned by `dy` (points for a trackpad's fine turn, `precise`, else steps of a
/// wheel), in whole rows `tall` points high; a fine turn's rest is kept in `rest` for the next.
pub(crate) fn wheel_rows(rest: &mut f64, dy: f64, precise: bool, tall: f64) -> i32 {
    let rows = if precise {
        *rest += dy / tall.max(1.0);
        let whole = rest.trunc();
        *rest -= whole;
        whole
    } else if dy == 0.0 {
        0.0
    } else {
        dy.signum() * dy.abs().round().max(1.0)
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a turn of the wheel, in rows, held to a thousand just before"
    )]
    let rows = rows.clamp(-1000.0, 1000.0) as i32;
    rows
}

/// An item of the terminal's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Picked {
    Copy,
    Paste,
    SelectAll,
    Clear,
}

/// The terminal's menu: Copy while something is selected, Paste, Select All, and Clear on the
/// main screen (a full-screen program draws all of its own); a line before Select All. Each
/// item's place among the items, with what it does.
pub(crate) fn menu(selected: bool, main: bool) -> (Vec<Entry>, Vec<Picked>) {
    let mut picks = vec![];
    if selected {
        picks.push(Picked::Copy);
    }
    picks.extend([Picked::Paste, Picked::SelectAll]);
    if main {
        picks.push(Picked::Clear);
    }
    let mut entries = vec![];
    for pick in &picks {
        if *pick == Picked::SelectAll {
            entries.push(Entry::Line { line: true });
        }
        let label = match pick {
            Picked::Copy => "Copy",
            Picked::Paste => "Paste",
            Picked::SelectAll => "Select All",
            Picked::Clear => "Clear",
        };
        entries.push(Entry::Item {
            label: label.into(),
            marked: false,
            off: false,
            icon: None,
            key: None,
        });
    }
    (entries, picks)
}

/// The cell of `frame` under `(x, y)` (the view's points from its top left), in cells `cell`
/// points large; none in the gutter.
pub(crate) fn cell_at(frame: &Frame, cell: (f64, f64), (x, y): (f64, f64)) -> Option<(u16, u16)> {
    let pad = layout::gutter(frame.alternate);
    let column = ((x - f64::from(pad.left)) / cell.0).floor();
    let row = ((y - f64::from(pad.top)) / cell.1).floor();
    let inside = column >= 0.0
        && row >= 0.0
        && column < f64::from(frame.columns)
        && row < f64::from(frame.lines);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a cell of the grid, held inside it just above"
    )]
    let found = inside.then_some((column as u16, row as u16));
    found
}

/// The cursor's cell as points from the view's top left (left, top, wide, tall): where an input
/// method's candidates stand.
pub(crate) fn cursor_cell(frame: &Frame, cell: (f64, f64)) -> Option<[f64; 4]> {
    let (column, row) = frame.cursor.at?;
    let pad = layout::gutter(frame.alternate);
    Some([
        f64::from(pad.left) + f64::from(column) * cell.0,
        f64::from(pad.top) + f64::from(row) * cell.1,
        cell.0,
        cell.1,
    ])
}

/// Whether `x` (points) is in the scrollbar's column of a view `width` points wide.
pub(crate) fn on_bar(x: f64, width: f64) -> bool {
    x >= width - SCROLLBAR_WIDTH
}

/// Where on the slider (`top`, `length`) a press at `y` takes it: where it was pressed, or its
/// middle, brought there, for a press in its track.
pub(crate) fn grip(y: f64, (top, length): (f64, f64)) -> f64 {
    match (top..top + length).contains(&y) {
        true => y - top,
        false => length / 2.0,
    }
}

/// The row of history at the top of the view when the slider, `length` long in a view `height`
/// tall and taken `grip` from its top, is dragged to `y`.
pub(crate) fn bar_row(y: f64, grip: f64, (length, height): (f64, f64), bar: Scrollbar) -> u64 {
    let room = (height - length).max(1.0);
    let share = ((y - grip) / room).clamp(0.0, 1.0);
    let rows = bar.total.saturating_sub(bar.len) as f64;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a row of history, held between none and the last just above"
    )]
    let row = (share * rows).round() as u64;
    row
}

/// The Option key the person chose as Meta, as the terminal is told it.
pub(crate) const fn meta(chosen: crate::browser::settings::OptionMeta) -> OptionMeta {
    use crate::browser::settings::OptionMeta as Chosen;
    match chosen {
        Chosen::Left => OptionMeta::Left,
        Chosen::Right => OptionMeta::Right,
        Chosen::Both => OptionMeta::Both,
        Chosen::Off => OptionMeta::Off,
    }
}

/// The view's place on the screen and in the window, as a look is made of it: dark or light, the
/// screen's pixels to a point, the system's accent, whether it has the keys in the key window
/// (`active`) and its window is the key one (`key`), and the system's display options as a
/// check set them (none: as the system says).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Where {
    pub(crate) dark: bool,
    pub(crate) scale: f64,
    pub(crate) accent: crate::surface::Rgb,
    pub(crate) active: bool,
    pub(crate) key: bool,
    pub(crate) display: Option<(bool, bool)>,
}

/// The look from the person's `settings` and where the view is: more contrast and a solid ground
/// as the system's display options say (a ground of its own wherever the window has no glass).
pub(crate) fn look(settings: &crate::browser::settings::Settings, at: Where) -> TermLook {
    use crate::browser::frame::{Transparency, transparency};
    let glass = cfg!(target_os = "macos") && transparency() == Transparency::Allowed;
    TermLook {
        dark: at.dark,
        font: settings.mono().into(),
        size: f64::from(settings.font_size),
        scale: at.scale,
        ligatures: settings.ligatures.on(),
        active: at.active,
        key: at.key,
        accent: at.accent,
        more: at
            .display
            .map_or_else(crate::browser::frame::increased, |(more, _)| more),
        solid: at.display.map_or(!glass, |(_, solid)| solid),
    }
}

/// The geometry of a view `size` points large at `scale`, of cells `cell` points, as the
/// terminal reads the pointer by it (pixels).
pub(crate) fn geometry(
    size: (f64, f64),
    scale: f64,
    cell: (f64, f64),
    alternate: bool,
) -> Geometry {
    let pad = layout::gutter(alternate);
    let px =
        |points: f64| u32::try_from(super::painter::pixels(points * scale)).unwrap_or(u32::MAX);
    Geometry {
        width: px(size.0),
        height: px(size.1),
        cell_width: px(cell.0),
        cell_height: px(cell.1),
        pad_top: px(f64::from(pad.top)),
        pad_bottom: px(f64::from(pad.bottom)),
        pad_left: px(f64::from(pad.left)),
        pad_right: px(f64::from(pad.right)),
    }
}

#[cfg(test)]
#[path = "acts_tests.rs"]
mod tests;
