//! Strict readers of trusted UI messages: clicks, shortcuts, dragging and coordinates.

use super::{Key, SIDE};
use crate::browser::agents::AGENTS;

/// A toolbar button the strip had no room for: it waits in the "»" menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum Tool {
    Events,
    History,
}

impl Tool {
    /// The buttons, in the toolbar's order (and the menu's).
    pub(in super::super) const ALL: [Self; 2] = [Self::Events, Self::History];

    /// The button the strip names by its id.
    fn parse(id: &str) -> Option<Self> {
        Some(match id {
            "ev" => Self::Events,
            "hs" => Self::History,
            _ => return None,
        })
    }

    /// Its line in the "»" menu.
    pub(in super::super) fn label(self) -> &'static str {
        match self {
            Self::Events => "Event Center",
            Self::History => "History",
        }
    }

    /// Its bit in `Click::More`'s set.
    pub(in super::super) fn bit(self) -> u8 {
        match self {
            Self::History => 1,
            Self::Events => 2,
        }
    }
}

/// What the person did on the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum Click {
    Show(Key),
    Close(Key),
    Back,
    Forward,
    /// The history menu opens or closes.
    History,
    /// The history menu's entry of this number.
    Visit(i64),
    /// The offer bar's button that does it (`true`), or the other.
    Offer(bool),
    /// The Event Center opens or closes.
    Events,
    /// The Event Center opens, or stays open (a notice of the system's was clicked).
    Listed,
    /// The open menu closes (a click beside it).
    Unmenu,
    /// Action `.1` of the Event Center's event numbered `.0`.
    Event(u64, u8),
    /// The Event Center cleared: all it shows dismissed.
    Clear,
    /// Commenting on the pages turns on or off.
    Comment,
    /// The comments held go to whoever listens, and their marks leave the pages.
    Send,
    /// The strip's empty part is the titlebar: a press moves the window, a double click zooms it.
    Drag,
    Zoom,
    /// The window is put away (its yellow button, ⌘M).
    Collapse,
    /// The app quits (asked first while programs run).
    Quit,
    /// The page shown loads again.
    Reload,
    /// The page shown opens in the person's default browser.
    Outside,
    /// The side, the tabs' column, slides shut or open.
    Side,
    /// The person drags the side's edge: its width now, in points, and whether they let go.
    SideWidth(u32, bool),
    /// The person drags the line between a split window's halves: by how many points since
    /// they took hold of it, and whether they let go.
    SplitMove(i32, bool),
    /// The line between the halves, double-clicked: the halves even again.
    SplitEven,
    /// The person dragged a tab along the list: it goes before the other (none: to the end).
    Move(Key, Option<Key>),
    /// A tab dragged out over the pages, at a point of the window: where it would go shows.
    DropOver(Key, u32, u32),
    /// The dragged tab back over the list (or let go there): nothing shows on the pages.
    DropOff,
    /// A tab let go of over the pages, at a point of the window: it goes into that half.
    Drop(Key, u32, u32),
    /// "+" at these points of the strip: a new page or terminal, as the person picks.
    NewTab(u32, u32),
    /// The size button at these points of the strip: the page's size, as the person picks.
    Frame(u32, u32),
    /// The project switcher at these points of the strip: a project, as the person picks.
    Projects(u32, u32),
    /// The shown tab's crumb at these points of the strip: the project's tabs, to show another.
    Tabs(u32, u32),
    /// "»" at these points of the strip, with the buttons it holds (`Tool::bit`s): one of them,
    /// as the person picks.
    More(u32, u32, u8),
    /// The strip's open list (the address field's suggestions, the history menu) is at this
    /// rectangle of it (x, y, width, height), or none is open: the system's glass goes under it.
    Glass(Option<(u32, u32, u32, u32)>),
    /// A tab's menu (a right click on it), at these points of the strip.
    Menu(Key, u32, u32),
    /// The address field opens (its suggestions need the window) or shuts.
    Typing(bool),
    /// A shortcut's doing that no button of the strip does (shortcuts.rs).
    Key(Keyed),
    /// A coding agent's mark where no tab is left: a terminal that runs it (its id, agents.rs).
    Agent(&'static str),
}

/// What a shortcut does that has no button of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum Keyed {
    /// The project's tab of this place, from 1; the last.
    Tab(u8),
    /// The project switcher's menu, under its button.
    Projects,
    /// A folder picked to open as a project.
    OpenFolder,
    /// The project shown closes: its tabs.
    CloseProject,
    /// The window splits, its tab shown beside another of its project's, or joins again.
    Split,
    LastTab,
    Next,
    Previous,
    NewWindow,
    NewPage,
    NewTerminal,
    NewBoard,
    /// The tab shown closes.
    Close,
    /// The tab closed last opens again.
    Reopen,
    /// The side's search takes the keyboard.
    Find,
    /// The address field takes the keyboard.
    Address,
    /// The page size menu, under its button.
    Size,
    Settings,
}

impl Keyed {
    fn parse(words: &str) -> Option<Self> {
        Some(match words {
            "tab last" => Self::LastTab,
            "split" => Self::Split,
            "next" => Self::Next,
            "previous" => Self::Previous,
            "new window" => Self::NewWindow,
            "new page" => Self::NewPage,
            "new terminal" => Self::NewTerminal,
            "new board" => Self::NewBoard,
            "close" => Self::Close,
            "reopen" => Self::Reopen,
            "find" => Self::Find,
            "address" => Self::Address,
            "size" => Self::Size,
            "settings" => Self::Settings,
            "projects" => Self::Projects,
            "open folder" => Self::OpenFolder,
            "close project" => Self::CloseProject,
            words => Self::Tab(
                words
                    .strip_prefix("tab ")?
                    .parse()
                    .ok()
                    .filter(|at| (1..=8).contains(at))?,
            ),
        })
    }
}

/// A drag's message after its name: how far (its first word), and whether the person let go
/// (`done` after it).
fn dragged(rest: &str) -> Option<(&str, bool)> {
    match rest.split_once(' ') {
        Some((far, "done")) => Some((far, true)),
        Some(_) => None,
        None => Some((rest, false)),
    }
}

/// The click on the mark of the agent `id`: only one the app knows.
fn agent(id: &str) -> Option<Click> {
    let known = AGENTS.iter().find(|agent| agent.id == id)?;
    Some(Click::Agent(known.id))
}

/// A message from the strip's page: `show w3`, `close t1`, `back`, `forward`, `history`,
/// `visit 2`, `comment`, `send`, `drag`, `zoom`, `collapse`. Anything else is nothing.
pub(in super::super) fn click(body: &str) -> Option<Click> {
    let (verb, rest) = match body.split_once(' ') {
        Some((verb, rest)) => (verb, Some(rest)),
        None => (body, None),
    };
    match (verb, rest) {
        ("back", None) => Some(Click::Back),
        ("forward", None) => Some(Click::Forward),
        ("history", None) => Some(Click::History),
        ("events", None) => Some(Click::Events),
        ("events", Some("open")) => Some(Click::Listed),
        ("unmenu", None) => Some(Click::Unmenu),
        ("clear", None) => Some(Click::Clear),
        ("event", Some(at)) => {
            let (event, action) = at.split_once(' ')?;
            Some(Click::Event(event.parse().ok()?, action.parse().ok()?))
        }
        ("comment", None) => Some(Click::Comment),
        ("send", None) => Some(Click::Send),
        ("drag", None) => Some(Click::Drag),
        ("zoom", None) => Some(Click::Zoom),
        ("collapse", None) => Some(Click::Collapse),
        ("quit", None) => Some(Click::Quit),
        ("reload", None) => Some(Click::Reload),
        ("outside", None) => Some(Click::Outside),
        ("side", None) => Some(Click::Side),
        ("split-even", None) => Some(Click::SplitEven),
        ("split-move", Some(rest)) => {
            let (by, done) = dragged(rest)?;
            by.parse().ok().map(|by| Click::SplitMove(by, done))
        }
        ("key", Some(words)) => Keyed::parse(words).map(Click::Key),
        ("agent", Some(id)) => agent(id),
        ("side-width", Some(rest)) => {
            let (width, done) = dragged(rest)?;
            // `usual`: the width the side has until the person drags it.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "SIDE is a few hundred whole points"
            )]
            let usual = SIDE as u32;
            match width {
                "usual" => Some(Click::SideWidth(usual, done)),
                width => width
                    .parse()
                    .ok()
                    .map(|width| Click::SideWidth(width, done)),
            }
        }
        ("new", Some(at)) => {
            let (x, y) = at.split_once(' ')?;
            Some(Click::NewTab(x.parse().ok()?, y.parse().ok()?))
        }
        ("frame", Some(at)) => {
            let (x, y) = at.split_once(' ')?;
            Some(Click::Frame(x.parse().ok()?, y.parse().ok()?))
        }
        ("tabs", Some(at)) => {
            let (x, y) = at.split_once(' ')?;
            Some(Click::Tabs(x.parse().ok()?, y.parse().ok()?))
        }
        ("projects", Some(at)) => {
            let (x, y) = at.split_once(' ')?;
            Some(Click::Projects(x.parse().ok()?, y.parse().ok()?))
        }
        ("more", Some(rest)) => {
            let mut parts = rest.split(' ');
            let x = parts.next()?.parse().ok()?;
            let y = parts.next()?.parse().ok()?;
            let mut held = 0;
            for id in parts {
                held |= Tool::parse(id)?.bit();
            }
            (held != 0).then_some(Click::More(x, y, held))
        }
        ("typing", Some("on")) => Some(Click::Typing(true)),
        ("typing", Some("off")) => Some(Click::Typing(false)),
        ("offer", Some("take")) => Some(Click::Offer(true)),
        ("offer", Some("later")) => Some(Click::Offer(false)),
        ("visit", Some(at)) if at.bytes().all(|b| b.is_ascii_digit()) => {
            at.parse().ok().map(Click::Visit)
        }
        ("glass", Some("none")) => Some(Click::Glass(None)),
        ("glass", Some(rest)) => {
            let mut parts = rest.split(' ');
            let mut next = || parts.next()?.parse().ok();
            let rect = (next()?, next()?, next()?, next()?);
            parts.next().is_none().then_some(Click::Glass(Some(rect)))
        }
        ("menu", Some(rest)) => {
            let mut parts = rest.split(' ');
            let key = Key::parse(parts.next()?)?;
            let x = parts.next()?.parse().ok()?;
            let y = parts.next()?.parse().ok()?;
            parts.next().is_none().then_some(Click::Menu(key, x, y))
        }
        ("show", Some(key)) => Key::parse(key).map(Click::Show),
        ("close", Some(key)) => Key::parse(key).map(Click::Close),
        (verb, rest) => dragged_tab(verb, rest),
    }
}

/// A tab the person drags: along the list (`move KEY BEFORE|end`), over the pages
/// (`drop-over KEY X Y`, `drop-off`), let go of over them (`drop KEY X Y`).
fn dragged_tab(verb: &str, rest: Option<&str>) -> Option<Click> {
    let point = |at: &str| -> Option<(u32, u32)> {
        let (x, y) = at.split_once(' ')?;
        Some((x.parse().ok()?, y.parse().ok()?))
    };
    match (verb, rest) {
        ("move", Some(rest)) => {
            let (key, before) = rest.split_once(' ')?;
            let before = match before {
                "end" => None,
                before => Some(Key::parse(before)?),
            };
            Some(Click::Move(Key::parse(key)?, before))
        }
        ("drop-off", None) => Some(Click::DropOff),
        ("drop-over", Some(rest)) => {
            let (key, at) = rest.split_once(' ')?;
            let (x, y) = point(at)?;
            Some(Click::DropOver(Key::parse(key)?, x, y))
        }
        ("drop", Some(rest)) => {
            let (key, at) = rest.split_once(' ')?;
            let (x, y) = point(at)?;
            Some(Click::Drop(Key::parse(key)?, x, y))
        }
        _ => None,
    }
}

/// What the person clicked in another part of the window (the side, the browser's toolbar), said
/// as the strip says it: its points moved by `(x, y)`, where that part lies in the window.
pub(in super::super) fn moved(body: &str, (x, y): (f64, f64)) -> String {
    let parts: Vec<&str> = body.split(' ').collect();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a point of the window, whole and positive"
    )]
    let at = |value: &str, by: f64| -> Option<String> {
        let value: f64 = value.parse().ok()?;
        Some(((value + by).max(0.0).round() as u32).to_string())
    };
    let point = |px: &str, py: &str| Some(format!("{} {}", at(px, x)?, at(py, y)?));
    let moved = match parts.as_slice() {
        ["menu", key, px, py] => point(px, py).map(|p| format!("menu {key} {p}")),
        ["drop", key, px, py] => point(px, py).map(|p| format!("drop {key} {p}")),
        ["drop-over", key, px, py] => point(px, py).map(|p| format!("drop-over {key} {p}")),
        [
            verb @ ("new" | "frame" | "projects" | "tabs"),
            px,
            py,
            rest @ ..,
        ] => point(px, py).map(|p| [&[*verb, p.as_str()][..], rest].concat().join(" ")),
        ["more", px, py, rest @ ..] => {
            point(px, py).map(|p| [&["more", p.as_str()][..], rest].concat().join(" "))
        }
        ["glass", px, py, w, h] => point(px, py).map(|p| format!("glass {p} {w} {h}")),
        _ => None,
    };
    moved.unwrap_or_else(|| body.to_string())
}
