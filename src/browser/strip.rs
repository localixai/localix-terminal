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

mod input;

pub(super) use input::{Click, Keyed, Tool, click, moved};

use serde_json::json;

use super::agents;

/// The bar's height, in points: the window's buttons, the open projects, where the shown tab is,
/// and the window's own buttons (a compact unified toolbar's, frame_macos.rs).
pub(super) const BAR: f64 = 38.0;
/// The browser's toolbar over a page, in points: back, forward, reload, the address field, the
/// page's size and comments. Only a page has it.
pub(super) const NAV: f64 = 40.0;
/// The tabs' row under the bar, in points, where there is no side.
const ROW: f64 = 32.0;
/// The strip's height, in points, where the tabs are a row under the toolbar (no side).
pub(super) const HEIGHT: f64 = BAR + ROW;
/// The side's width while it is open, in points, until the person drags it wider or narrower:
/// the tabs listed down the window's left.
pub(super) const SIDE: f64 = 248.0;
/// The side shut: a rail of the tabs' pictures, its button where it was.
pub(super) const RAIL: f64 = 52.0;
/// The narrowest and widest the person can drag the side; never more than half the window.
pub(super) const SIDE_MIN: f64 = 200.0;
pub(super) const SIDE_MAX: f64 = 480.0;

/// The widest the side may be in a window `window` points wide (none: in any window):
/// `SIDE_MAX`, held to half the window, never under `SIDE_MIN`.
pub(super) fn side_widest(window: Option<f64>) -> f64 {
    window
        .map_or(SIDE_MAX, |window| SIDE_MAX.min(window / 2.0))
        .max(SIDE_MIN)
}

/// A width the side is given (dragged, or kept from another window), held between `SIDE_MIN`
/// and the widest it may be in `window`.
pub(super) fn side_held(width: f64, window: Option<f64>) -> f64 {
    width.clamp(SIDE_MIN, side_widest(window))
}

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
    let places: Vec<_> = (projects.places.iter())
        .map(|(place, marked)| {
            json!({"key": place.key, "name": place.name, "marked": marked,
                "folder": place.folder.display().to_string()})
        })
        .collect();
    format!(
        "S({})",
        json!({"tabs": tabs, "shown": shown.name(), "side": side,
            "project": projects.shown, "places": places})
    )
}

/// The call that tells the browser's toolbar the pages the project showed, newest first: what
/// its address field suggests. The toolbar's alone: the strip and the side never read them.
pub(super) fn visited(visited: &[super::Visited]) -> String {
    let visited: Vec<_> = (visited.iter())
        .map(|page| json!({"title": page.title, "url": page.url}))
        .collect();
    format!("VI({})", json!(visited))
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

/// The Event Center as the strip shows it: `events` (a JSON list, newest first), and the list itself while it is `open`.
pub(super) fn center(events: &str, open: bool) -> String {
    let events = if events.is_empty() { "[]" } else { events };
    format!("V({events}, {open})")
}

/// The call that marks the comment button on or off, and offers to send the comments held.
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

/// The call that turns commenting on or off in a page, with the run's call `key`; a page whose
/// marks are of an earlier batch than `sent` drops them. Nobody waits for its reply: call 0,
/// whose reply no call takes.
pub(super) fn comment(key: &str, on: bool, sent: u32) -> String {
    format!(
        "{}, 'comment', {{ on: {on}, sent: {sent} }}) : false",
        super::script::head(key, 0, "")
    )
}

/// The strip's page: dressed in the frame's stylesheet, clear over the window's glass.
pub(super) const PAGE: &str = include_str!("strip.html");

/// The SHA-384 of the page's one script, its resize helper in: the page's policy runs it alone.
const SCRIPT_SHA384: &str = "F3c6EPTEYsZR3cG+SejcNRoHl5HoYsq6Fuj41wEXTVwIEW25u7f3mvimB4wuNqrP";

/// The strip's page: the trusted UI's resize helper inlined, so it opens without external
/// resources, its bars as tall as the window lays them out (`BAR`, `NAV`, `ROW`: its style, not
/// its script), and the agents' `marks` (JSON) in it as data its script reads, never as script.
pub(super) fn page(marks: &str) -> String {
    // Inside a script element only `</script` ends it: no `<` of the data reaches the parser.
    PAGE.replacen("/*RESIZE_HANDLES*/", include_str!("resize.js"), 1)
        .replacen("{{hash}}", SCRIPT_SHA384, 1)
        .replacen("{{bar}}", &BAR.to_string(), 1)
        .replacen("{{nav}}", &NAV.to_string(), 1)
        .replacen("{{row}}", &ROW.to_string(), 1)
        .replacen(
            "id=\"marks\">{}<",
            &format!("id=\"marks\">{}<", marks.replace('<', "\\u003c")),
            1,
        )
}

#[cfg(test)]
#[path = "strip_tests.rs"]
mod tests;
