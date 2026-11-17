//! A tab carried from a side's list, as one state for the whole gesture (docs/windows.md): lifted
//! in its list, over its window's panes (standing where it would land, `deck_preview.rs`), out
//! of its window (a window of its own that follows the pointer, `host_tear.rs`), and let go.
//! What the side's page says of it, read here, and what each word does in each state: plain
//! data, so the loop (`host_carry.rs`) only carries out what this says. Nothing is written to
//! the layout before the tab is let go.

use super::strip::Key;

/// A point, in points.
pub(super) type Point = (f64, f64);

/// What the side's page says of a carried tab.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Said<'a> {
    /// Taken out of the list: its pane (none for an entry of the files' tree), where in its card
    /// the pointer holds it, the card's place in the page, and the page's height.
    Lift {
        key: Option<Key>,
        grab: Point,
        rect: (f64, f64, f64, f64),
        inner: Option<f64>,
    },
    /// The pointer on the screen.
    At(Point),
    /// Over the pages, or out of the window: its point in the page, and on the screen.
    Over {
        key: Key,
        at: Point,
        screen: Option<Point>,
    },
    /// Let go over the pages or out of the window.
    Drop {
        key: Key,
        at: Point,
        screen: Option<Point>,
    },
    /// Back over its list, or Escape.
    Off,
    /// The gesture ended.
    End,
    /// The tab of the slot another window's list opened (none: its end).
    Slot(Option<&'a str>),
}

/// Where the carried tab is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    /// In its list, which moves it.
    Lifted,
    /// Over its window's panes.
    Over,
    /// Out of its window, in a window that follows the pointer.
    Out,
    /// Let go; the gesture's end is still to come.
    Landed,
}

/// What a word of the page does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Do {
    /// Its card drawn at the pointer, and its window made ready for it to land.
    Lift,
    /// Its card follows the pointer.
    Follow,
    /// It came over the panes: their zones taken, then aimed.
    Enter,
    /// The pointer moved over the panes: where it lands looked for again.
    Aim,
    /// Off the panes: the panes back as they lay.
    Leave,
    /// Past the window's edge: the panes back as they lay, then out into a window of its own.
    ComeOut,
    /// The window it is carried in follows the pointer.
    Move,
    /// Let go over the panes: the layout told where, drawn once.
    Land,
    /// Let go out of its window: into the list it is over, or its window stays.
    LetGo,
    /// Back over its own list: its window gives it back.
    TakeBack,
    /// The slot of the list it is over.
    Slot,
    /// The gesture is over.
    End,
    Nothing,
}

/// What `said` does to a tab carried in `now` (none: no tab is carried), and the state after:
/// `past` says the pointer is out past its window's edge.
pub(super) fn next(now: Option<Phase>, said: &Said<'_>, past: bool) -> (Option<Phase>, Do) {
    use Phase::{Landed, Lifted, Out, Over};
    let Some(phase) = now else {
        return match said {
            Said::Lift { .. } => (Some(Lifted), Do::Lift),
            Said::At(_)
            | Said::Over { .. }
            | Said::Drop { .. }
            | Said::Off
            | Said::End
            | Said::Slot(_) => (None, Do::Nothing),
        };
    };
    match (phase, said) {
        (_, Said::Lift { .. }) => (Some(Lifted), Do::Lift),
        (_, Said::At(_)) => (now, Do::Follow),
        (Lifted | Over, Said::Over { .. }) if past => (Some(Out), Do::ComeOut),
        (Lifted, Said::Over { .. }) => (Some(Over), Do::Enter),
        (Over, Said::Over { .. }) => (Some(Over), Do::Aim),
        (Out, Said::Over { .. }) => (Some(Out), Do::Move),
        (Over, Said::Off) => (Some(Lifted), Do::Leave),
        (Out, Said::Off) => (Some(Lifted), Do::TakeBack),
        (Over, Said::Drop { .. }) => (Some(Landed), Do::Land),
        (Out, Said::Drop { .. }) => (Some(Landed), Do::LetGo),
        (Lifted | Landed, Said::Drop { .. }) => (Some(Landed), Do::Nothing),
        (Out, Said::Slot(_)) => (now, Do::Slot),
        // Ended with no word of where it went: the panes back as they were, a window that
        // follows the pointer left where it is.
        (Over, Said::End) => (None, Do::Leave),
        (Out, Said::End) => (None, Do::LetGo),
        (Lifted | Landed, Said::End) => (None, Do::End),
        (Lifted | Landed, Said::Off | Said::Over { .. }) | (_, Said::Slot(_)) => (now, Do::Nothing),
    }
}

/// The page's word `message`, when it is about a carried tab.
pub(super) fn said(message: &str) -> Option<Said<'_>> {
    let parts: Vec<&str> = message.split(' ').collect();
    let number = |part: &str| part.parse::<f64>().ok();
    let point = |x: &str, y: &str| Some((number(x)?, number(y)?));
    match parts.as_slice() {
        ["drag-end"] => Some(Said::End),
        ["drop-off"] => Some(Said::Off),
        ["slot", before] => Some(Said::Slot((*before != "end").then_some(*before))),
        ["drag-at", x, y] => Some(Said::At(point(x, y)?)),
        ["drag-start", rest @ ..] => lift(rest),
        [verb @ ("drop-over" | "drop"), key, x, y, rest @ ..] => {
            let key = Key::parse(key)?;
            let at = point(x, y)?;
            let screen = match rest {
                [] => None,
                [sx, sy] => Some(point(sx, sy)?),
                _ => return None,
            };
            Some(match *verb {
                "drop" => Said::Drop { key, at, screen },
                _ => Said::Over { key, at, screen },
            })
        }
        _ => None,
    }
}

/// `drag-start [KEY] GX GY X Y W H [IH]`: a tab's card names its pane first, a tree entry's
/// does not.
fn lift<'a>(parts: &[&str]) -> Option<Said<'a>> {
    let (key, numbers) = match parts.first().map(|first| Key::parse(first)) {
        Some(Some(key)) => (Some(key), parts.get(1..)?),
        _ => (None, parts),
    };
    let numbers: Option<Vec<f64>> = numbers.iter().map(|part| part.parse().ok()).collect();
    let (grab, rect, inner) = match numbers?[..] {
        [gx, gy, x, y, width, height] => ((gx, gy), (x, y, width, height), None),
        [gx, gy, x, y, width, height, inner] => ((gx, gy), (x, y, width, height), Some(inner)),
        _ => return None,
    };
    Some(Said::Lift {
        key,
        grab,
        rect,
        inner,
    })
}

#[cfg(test)]
#[path = "carry_tests.rs"]
mod tests;
