//! The strip along the top of the one window, and the tabs: the browser's tabs, the terminal
//! sessions' screens and the board, the one shown marked. The person switches with a click and
//! closes one with its ×; the page says which as one short message, read as data. Titles are
//! what pages and commands say, so they reach the page as JSON and are set as text. With the tabs
//! of several projects in it (one window, several sessions), a row of the projects goes in the
//! strip, and the tabs listed are the shown project's.
//!
//! The tabs are a column on the window's left (the side), the same page in a view of its own,
//! which the person slides shut and open; where the pages lie in a column (GTK), a row of them
//! under the strip.

use serde_json::json;

use super::agents;

/// The bar's height, in points: the window's buttons, the open projects, where the shown tab is,
/// and the window's own buttons (a compact unified toolbar's, frame_macos.rs).
pub(super) const BAR: f64 = 38.0;
/// The browser's toolbar over a page, in points: back, forward, reload, the address field, the
/// page's size and comments. Only a page has it.
pub(super) const NAV: f64 = 40.0;
/// The strip's height, in points, where the tabs are a row under the toolbar (no side).
pub(super) const HEIGHT: f64 = BAR + 32.0;
/// The side's width while it is open, in points, until the person drags it wider or narrower:
/// the tabs listed down the window's left.
pub(super) const SIDE: f64 = 264.0;
/// The side shut: a rail of the tabs' pictures, its button where it was.
pub(super) const RAIL: f64 = 52.0;
/// The narrowest and widest the person can drag the side; never more than half the window.
pub(super) const SIDE_MIN: f64 = 180.0;
pub(super) const SIDE_MAX: f64 = 480.0;

/// Panels past this id are boards: board `b1` is panel `BOARD + 1`, a terminal's screen is its
/// session's id.
pub const BOARD: u32 = 1 << 24;
/// The panel of the window's own Settings, a tab like a terminal's (`s1`): the place between the
/// terminals and the boards.
pub(super) const SETTINGS: u32 = BOARD;

/// A tab: one of the browser's (its id) or a panel (its id): a terminal session's screen, or a
/// board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Key {
    Web(u32),
    Term(u32),
}

impl Key {
    pub(super) fn name(self) -> String {
        match self {
            Key::Web(id) => format!("w{id}"),
            Key::Term(id) if id > BOARD => format!("b{}", id - BOARD),
            Key::Term(SETTINGS) => "s1".to_string(),
            Key::Term(id) => format!("t{id}"),
        }
    }

    /// What the strip draws it as: `web`, `term` or `board`.
    pub(super) fn kind(self) -> &'static str {
        match self {
            Key::Web(_) => "web",
            Key::Term(id) if id > BOARD => "board",
            Key::Term(SETTINGS) => "settings",
            Key::Term(_) => "term",
        }
    }

    pub(super) fn parse(text: &str) -> Option<Self> {
        let (kind, id) = text.split_at_checked(1)?;
        let id: u32 = id
            .parse()
            .ok()
            .filter(|_| id.bytes().all(|b| b.is_ascii_digit()))?;
        match kind {
            "w" => Some(Key::Web(id)),
            "t" if id < BOARD => Some(Key::Term(id)),
            "s" if id == 1 => Some(Key::Term(SETTINGS)),
            "b" if id > 0 => BOARD.checked_add(id).map(Key::Term),
            _ => None,
        }
    }
}

/// A toolbar button the strip had no room for: it waits in the "»" menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tool {
    Split,
    History,
    Mini,
    Events,
}

impl Tool {
    /// The buttons, in the toolbar's order (and the menu's).
    pub(super) const ALL: [Self; 4] = [Self::Split, Self::History, Self::Mini, Self::Events];

    /// The button the strip names by its id.
    fn parse(id: &str) -> Option<Self> {
        Some(match id {
            "sp" => Self::Split,
            "hs" => Self::History,
            "mn" => Self::Mini,
            "ev" => Self::Events,
            _ => return None,
        })
    }

    /// Its line in the "»" menu.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Split => "Split View",
            Self::History => "History",
            Self::Mini => "Mini Player",
            Self::Events => "Events",
        }
    }

    /// Its bit in `Click::More`'s set.
    pub(super) fn bit(self) -> u8 {
        match self {
            Self::History => 1,
            Self::Mini => 2,
            Self::Events => 4,
            Self::Split => 8,
        }
    }
}

/// What the person did on the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Click {
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
    /// An agent started in a terminal of the project shown: its place in the agents' list.
    Agent(u8),
    /// The Event Center opens or closes.
    Events,
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
    /// The expanded window goes back to the mini.
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
    DropOver(u32, u32),
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
}

/// What a shortcut does that has no button of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Keyed {
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

/// A message from the strip's page: `show w3`, `close t1`, `back`, `forward`, `history`,
/// `visit 2`, `comment`, `send`, `drag`, `zoom`, `collapse`. Anything else is nothing.
/// A drag's message after its name: how far (its first word), and whether the person let go
/// (`done` after it).
fn dragged(rest: &str) -> Option<(&str, bool)> {
    match rest.split_once(' ') {
        Some((far, "done")) => Some((far, true)),
        Some(_) => None,
        None => Some((rest, false)),
    }
}

pub(super) fn click(body: &str) -> Option<Click> {
    let (verb, rest) = match body.split_once(' ') {
        Some((verb, rest)) => (verb, Some(rest)),
        None => (body, None),
    };
    match (verb, rest) {
        ("back", None) => Some(Click::Back),
        ("forward", None) => Some(Click::Forward),
        ("history", None) => Some(Click::History),
        ("events", None) => Some(Click::Events),
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
        ("agent", Some(at)) => at.parse().ok().map(Click::Agent),
        ("visit", Some(at)) if at.bytes().all(|b| b.is_ascii_digit()) => {
            at.parse().ok().map(Click::Visit)
        }
        ("glass", Some("none")) => Some(Click::Glass(None)),
        ("glass", Some(rest)) => {
            let mut parts = rest.split(' ').map(str::parse);
            let mut next = || parts.next()?.ok();
            let rect = (next()?, next()?, next()?, next()?);
            next().is_none().then_some(Click::Glass(Some(rect)))
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
/// (`drop-over X Y`, `drop-off`), let go of over them (`drop KEY X Y`).
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
        ("drop-over", Some(at)) => point(at).map(|(x, y)| Click::DropOver(x, y)),
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
pub(super) fn moved(body: &str, (x, y): (f64, f64)) -> String {
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
        ["drop-over", px, py] => point(px, py).map(|p| format!("drop-over {p}")),
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

/// Where the search goes: what is typed in the address field that is not an address.
pub(super) const SEARCH: &str = "https://www.google.com/search";

/// Where a page the person opens with "+" starts: the search page's home.
pub const HOME: &str = "https://www.google.com";

/// Where `go <typed>`, what the person typed in the address field, goes: a web address as typed
/// (`https://` put before a bare one), else a search for it. Never a script or a file: only
/// http and https are gone to.
pub(super) fn went(body: &str) -> Option<String> {
    let typed = body.strip_prefix("go ")?.trim();
    if typed.is_empty() {
        return None;
    }
    let web = typed.starts_with("http://") || typed.starts_with("https://");
    let bare = !web && !typed.contains(char::is_whitespace) && !typed.contains("://");
    let local = typed.starts_with("localhost") || typed.starts_with("127.0.0.1");
    let address = match (web, bare) {
        (true, _) => Some(typed.to_string()),
        (false, true) if local => Some(format!("http://{typed}")),
        (false, true) if typed.contains('.') && !typed.starts_with('.') => {
            Some(format!("https://{typed}"))
        }
        (false, _) => None,
    };
    address
        .and_then(|address| super::guard::loadable(&address))
        .or_else(|| {
            url::Url::parse_with_params(SEARCH, [("q", typed)])
                .ok()
                .map(String::from)
        })
}

/// A tab as the strip lists it.
pub(super) struct Item {
    pub key: Key,
    pub title: String,
    /// A browser tab's page, or what a terminal's program says it is doing.
    pub detail: String,
    /// The project it was opened for, as the app names it.
    pub group: String,
    /// The coding agent a terminal runs in front (offer.rs), when it is known.
    pub agent: Option<&'static str>,
}

/// `items` with each group's together, the groups in the order they first appear.
pub(super) fn by_group<T>(mut items: Vec<T>, group: impl Fn(&T) -> &str) -> Vec<T> {
    let mut order: Vec<String> = vec![];
    for item in &items {
        if !order.iter().any(|seen| seen == group(item)) {
            order.push(group(item).to_string());
        }
    }
    items.sort_by_key(|item| order.iter().position(|seen| seen == group(item)));
    items
}

/// The projects as the switcher shows them: the one shown (its key), and each one the window
/// knows with whether something happened in it out of sight.
pub(super) struct Projects<'a> {
    pub shown: &'a str,
    pub places: Vec<(&'a super::Place, bool)>,
}

/// The call that draws the strip: the tabs in order, each with its project's key (the page lists
/// the shown project's, and the window's own), and the one shown; the projects, for the
/// switcher. `side`: whether the tabs' column is open, none where the window has no side (its
/// tabs are a row under the strip).
pub(super) fn script(
    items: &[Item],
    shown: Key,
    visited: &[super::Visited],
    side: Option<bool>,
    projects: &Projects<'_>,
) -> String {
    let tabs: Vec<_> = items
        .iter()
        .map(|item| {
            let kind = item.key.kind();
            let empty = match kind {
                "web" => "New tab",
                "board" => "Board",
                _ => "Terminal",
            };
            let title = item.title.trim();
            let title = if title.is_empty() { empty } else { title };
            let mut tab =
                json!({"key": item.key.name(), "kind": kind, "title": title, "detail": item.detail});
            // A terminal that runs a coding agent shows the agent's mark and name: the one in
            // front (typed in its shell too), else the one it was opened with.
            let running = item.agent.and_then(|id| agents::AGENTS.iter().find(|a| a.id == id));
            let agent = (kind == "term")
                .then(|| running.or_else(|| title.rsplit(" · ").next().and_then(agents::of_command)))
                .flatten();
            if let Some(agent) = agent {
                tab["agent"] = json!(agent.id);
                tab["name"] = json!(agent.name);
            }
            tab["group"] = json!(item.group);
            tab
        })
        .collect();
    let visited: Vec<_> = visited
        .iter()
        .map(|page| json!({"title": page.title, "url": page.url}))
        .collect();
    let places: Vec<_> = (projects.places.iter())
        .map(|(place, marked)| {
            json!({"key": place.key, "name": place.name, "marked": marked,
                "folder": place.folder.display().to_string()})
        })
        .collect();
    format!(
        "S({})",
        json!({"tabs": tabs, "shown": shown.name(), "visited": visited, "side": side,
            "project": projects.shown, "places": places})
    )
}

/// What became of a tab the history menu lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Open,
    /// A terminal the person closed: its program runs on, its screen live out of sight.
    Hidden,
    /// A terminal whose session ended: its last screen is kept.
    Ended,
    /// A browser tab closed: its page opens again in a new one.
    Closed,
}

/// An entry of the history menu: its row in the project's ledger, and its tab's key while open.
pub(super) struct Row {
    pub at: i64,
    pub key: Option<Key>,
    /// What it is, as the strip names it: `web`, `term` or `board`.
    pub kind: &'static str,
    pub title: String,
    /// A browser tab's page.
    pub detail: String,
    pub state: State,
    pub group: String,
}

/// The call that opens the history menu on `rows` (newest first), or closes it.
pub(super) fn menu(rows: Option<&[Row]>) -> String {
    let shown = rows.map(|rows| {
        let rows: Vec<_> = rows
            .iter()
            .map(|row| {
                let state = match row.state {
                    State::Open => "open",
                    State::Hidden => "hidden",
                    State::Ended => "ended",
                    State::Closed => "closed",
                };
                json!({"at": row.at, "key": row.key.map(Key::name), "kind": row.kind, "title": row.title,
                    "detail": row.detail, "state": state, "group": row.group})
            })
            .collect();
        rows
    });
    format!("M({})", json!(shown))
}

/// The call that marks the comment button on or off, and offers to send the comments held.
/// The Event Center as the strip shows it: `events` (a JSON list, newest first), its bell's count
/// of those not seen yet, and the list itself while it is `open`.
pub(super) fn center(events: &str, open: bool) -> String {
    let events = if events.is_empty() { "[]" } else { events };
    format!("V({events}, {open})")
}

pub(super) fn commenting(on: bool, held: usize) -> String {
    format!("C({on}, {held})")
}

/// Whose turn the strip shows: the agent's while it acts, the person's while they comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Turn {
    /// Nothing to show.
    Idle,
    /// The agent is acting on a page.
    Acting,
    /// The person comments: the agent's actions wait.
    Paused,
    /// The person comments, and an action of the agent's waits for them now.
    Waiting,
}

impl Turn {
    /// The turn from what the deck knows: an action already under way shows until it settles.
    pub(super) fn of(busy: bool, commenting: bool, waiting: usize) -> Self {
        match (busy, commenting, waiting) {
            (true, _, _) => Self::Acting,
            (false, true, 0) => Self::Paused,
            (false, true, _) => Self::Waiting,
            (false, false, _) => Self::Idle,
        }
    }
}

/// The call that shows whose turn it is.
pub(super) fn turn(turn: Turn) -> String {
    let what = match turn {
        Turn::Idle => "",
        Turn::Acting => "acting",
        Turn::Paused => "paused",
        Turn::Waiting => "waiting",
    };
    format!("A({})", json!(what))
}

/// The call that turns commenting on or off in a page; a page whose marks are of an earlier
/// batch than `sent` drops them.
pub(super) fn comment(on: bool, sent: u32) -> String {
    format!(
        "window.__dloop_call && window.__dloop_call(0, 'comment', {{ on: {on}, sent: {sent} }})"
    )
}

/// The strip's page: dressed in the frame's stylesheet, clear over the window's glass.
pub(super) const PAGE: &str = include_str!("strip.html");

#[cfg(test)]
#[path = "strip_tests.rs"]
mod tests;
