//! The `window` tool's actions: open, interact, screenshot, close, show and list, over
//! the owner's tabs.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::args::{TERM_FIELDS, WEB_FIELDS, picked, strings, text};
use super::center::Subject;
use super::{Call, Desk, Failure, Output};

/// What a tab is, from its key.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Web,
    Term,
    Board,
}

impl Kind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Web => "browser",
            Self::Term => "terminal",
            Self::Board => "board",
        }
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Web => &WEB_FIELDS,
            Self::Term => &TERM_FIELDS,
            Self::Board => &[],
        }
    }

    /// Every field `interact` reads on a tab of this kind, past those of any tab ([`ANY_TAB`]).
    fn takes(self) -> Vec<String> {
        let own = self.fields().iter().map(|field| (*field).to_string());
        match self {
            Self::Web => own.chain(DIALOG.map(String::from)).collect(),
            Self::Term => own.collect(),
            Self::Board => super::args::board_fields()
                .into_iter()
                .map(|(name, _)| name)
                .collect(),
        }
    }

    /// How what was meant is done on this kind, after a field it does not take.
    fn instead(self) -> &'static str {
        match self {
            Self::Web => {
                "A control is acted on by its ref from the outline (`[e12]`): `click` it, `fill` \
                 fields (`{\"e4\": \"text\"}`), `select` an option; keys are `keys`, anything \
                 else a `script`."
            }
            Self::Term => "Text is `input`, keys are `keys` ([\"Enter\"], [\"Ctrl-C\"]).",
            Self::Board => "A board is acted on with an `op`: add, take, review, block, release…",
        }
    }
}

/// The fields `interact` reads on any tab: which, the ask of the person, and who acts (a
/// subagent's name, as the caller gives it).
const ANY_TAB: [&str; 4] = ["action", "tab", "ask", "who"];
/// How a page's dialogs are answered.
const DIALOG: [&str; 2] = ["dialog", "dialog_text"];

/// A field `interact` would not read on `kind` is refused, never passed over: a page's
/// `"op": "click"` did nothing, and read as a page that would not answer a click. An `op` that
/// names a field the call carries (`"op": "script"` beside its `script`) says what the call
/// does already, and is let be.
fn unread(kind: Kind, args: &Value) -> Result<(), String> {
    let takes = kind.takes();
    let Some(given) = args.as_object() else {
        return Ok(());
    };
    let named = |field: &str| {
        field == "op"
            && kind != Kind::Board
            && (given.get("op").and_then(Value::as_str)).is_some_and(|op| {
                op != "op" && given.contains_key(op) && takes.iter().any(|t| t == op)
            })
    };
    let stray: Vec<String> = (given.keys())
        .filter(|field| !ANY_TAB.contains(&field.as_str()) && !takes.contains(field))
        .filter(|field| !named(field))
        .map(|field| format!("`{field}`"))
        .collect();
    if stray.is_empty() {
        return Ok(());
    }
    let listed: Vec<String> = takes.iter().map(|field| format!("`{field}`")).collect();
    Err(format!(
        "a {} tab takes no {}: nothing was done. It takes {}. {}",
        kind.name(),
        stray.join(", "),
        listed.join(", "),
        kind.instead()
    ))
}

/// What a board answers where a page or a terminal acts.
const ON_A_BOARD: &str = "a board is acted on with an `op` (add, take, review…), not keys";

/// A tab of the owner's: its key, kind and number.
pub(super) struct Tab {
    pub(super) key: String,
    pub(super) kind: Kind,
    pub(super) id: u32,
}

/// `w3` or `t2` as a tab.
pub(super) fn parse(key: &str) -> Option<Tab> {
    let key = key.trim().to_ascii_lowercase();
    let kind = match key.chars().next()? {
        'w' => Kind::Web,
        't' => Kind::Term,
        'b' => Kind::Board,
        _ => return None,
    };
    let id = key.get(1..)?.parse().ok()?;
    Some(Tab { key, kind, id })
}

impl Desk {
    pub(super) fn act(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        super::args::check(args)?;
        match text(args, "action") {
            Some("open") => self.open(call, args),
            Some("interact") => self.interact(call, args),
            Some("screenshot") => {
                let tab = self.tab(call, text(args, "tab"))?;
                Ok(match tab.kind {
                    Kind::Web => self.shoot_web(tab.id, text(args, "selector"))?,
                    Kind::Term => Output {
                        report: "A terminal is read as text: its screen is below.".into(),
                        state: Some(self.session(tab.id)?.snapshot()),
                        shot: None,
                    },
                    Kind::Board => Output {
                        report: "A board is read as text: it is below.".into(),
                        state: Some(self.board_text(&tab.key)?),
                        shot: None,
                    },
                })
            }
            Some("close") => self.close(call, args),
            Some("show") => {
                let tab = self.tab(call, text(args, "tab"))?;
                // A board shown is opened again when the window went with its tab.
                if tab.kind == Kind::Board {
                    return self.board(call, &json!({"op": "show"}));
                }
                let browser = self
                    .browser
                    .as_ref()
                    .ok_or_else(|| "there is no window here to show it in".to_string())?;
                browser.act(&tab.key, false)?;
                // The layout's too: its tab the one its workspace shows, the pane in focus.
                let row = crate::lock(&self.ledger).get(&tab.key).map(|(row, _)| *row);
                if let Some(Err(why)) = row.map(|row| self.show_pane(row)) {
                    log::warn!("{} is not shown in the layout: {why}", tab.key);
                }
                let read = self.read(&tab)?;
                Ok(Output {
                    report: format!("Showing {}.", tab.key),
                    ..read
                })
            }
            Some("list") => Ok(self.list(call.owner).into()),
            // The window is the person's: where it is and how large stay theirs. A page takes a
            // size of its own instead, framed in it.
            Some("resize") => Err(
                "the window is the person's and keeps its size: give a page a \
                 size of its own with `interact` and `size` (`mobile`, `tablet`, `Desktop`, \
                 `393x852`; `window` to fill it again)"
                    .to_string()
                    .into(),
            ),
            _ => Err(
                "`action` is one of open, interact, screenshot, close, show, list"
                    .to_string()
                    .into(),
            ),
        }
    }

    /// The owner's tab named `key`.
    fn tab(&self, call: &Call, key: Option<&str>) -> Result<Tab, String> {
        let key = key.ok_or("`tab` is needed: an id from `open` or `list`")?;
        let tab =
            parse(key).ok_or_else(|| format!("`{key}` is not a tab id: they look like w1, t2"))?;
        if tab.kind == Kind::Web {
            self.prune_pages();
        }
        match crate::lock(&self.registry).owner_of(&tab.key) {
            Some(owner) if owner == call.owner => Ok(tab),
            // A call from another directory is another owner's: say whose, so the agent sees it
            // left its project (a `cd` into a scratch directory) rather than lost the tab.
            _ => Err(format!(
                "there is no tab {} among yours, {}: `list` names them. Tabs are kept per \
                 project, the directory with `.git` around where the call runs (or \
                 LLX_DESK_PROJECT, or `--as`): run it from the project's directory",
                tab.key, call.owner
            )),
        }
    }

    /// Tab `tab` as it is now.
    pub(super) fn read(&self, tab: &Tab) -> Result<Output, String> {
        match tab.kind {
            Kind::Web => {
                let read = self.in_tab(tab.id, &json!({}), None)?;
                Ok(self.read_whole(tab.id, read))
            }
            Kind::Term => Ok(Output {
                state: Some(self.session(tab.id)?.snapshot()),
                ..Output::default()
            }),
            Kind::Board => Ok(Output {
                state: Some(self.board_text(&tab.key)?),
                ..Output::default()
            }),
        }
    }

    /// Acting in a tab, a page answering its dialogs as the call asks; what the page told
    /// during the call after the report.
    fn interact(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        // An ask with no tab: the session waits for the person.
        if text(args, "tab").is_none() && args.get("ask").is_some() {
            let session = Subject::Session(call.owner.to_string());
            return Ok(self.ask(call.owner, session, text(args, "ask")).into());
        }
        let tab = self.tab(call, text(args, "tab"))?;
        unread(tab.kind, args)?;
        // An ask of the person, on any kind of tab, and then what the call does there.
        let asked = (args.get("ask")).map(|_| {
            let on = Subject::Tab(tab.key.clone());
            self.ask(call.owner, on, text(args, "ask"))
        });
        let mut done = self.interact_tab(call, &tab, args)?;
        if let Some(asked) = asked {
            done.report = format!("{asked}\n\n{}", done.report);
        }
        Ok(done)
    }

    fn interact_tab(&self, call: &Call, tab: &Tab, args: &Value) -> Result<Output, Failure> {
        match tab.kind {
            Kind::Term => return self.interact_in(call, tab, args),
            // A board's `op` acts on it; with none, it is read (what a page or a terminal is
            // acted on with was refused already).
            Kind::Board => return self.board(call, args),
            Kind::Web => {}
        }
        let mark = self.told_mark();
        let answering = self.answering(tab.id, args)?;
        let mut done = self.interact_in(call, tab, args)?;
        drop(answering);
        if let Some(told) = self.told(tab.id, mark) {
            done.report = format!(
                "{}

{told}",
                done.report
            );
        }
        Ok(done)
    }

    /// Acting in a tab: the kind's own fields.
    fn interact_in(&self, call: &Call, tab: &Tab, args: &Value) -> Result<Output, Failure> {
        let mut done = self.interact_kind(call, tab, args)?;
        done.report = format!("{}: {}", tab.key, done.report);
        Ok(done)
    }

    fn interact_kind(&self, call: &Call, tab: &Tab, args: &Value) -> Result<Output, String> {
        match tab.kind {
            Kind::Web => self.interact_web(call, tab.id, args),
            Kind::Term => self.interact_term(tab.id, &picked(args, &TERM_FIELDS)),
            Kind::Board => Err(ON_A_BOARD.into()),
        }
    }

    fn close(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        let mut keys = strings(args, "tabs");
        keys.extend(text(args, "tab").map(String::from));
        if keys.is_empty() {
            return Err("`tab` or `tabs` is needed: ids from `open` or `list`"
                .to_string()
                .into());
        }
        let said: Vec<String> = keys
            .iter()
            .map(|key| {
                let done = self
                    .tab(call, Some(key))
                    .and_then(|tab| self.close_tab(&tab.key));
                done.unwrap_or_else(|why| format!("{key}: {why}"))
            })
            .collect();
        Ok(said.join("\n").into())
    }

    /// Close tab `key` and forget it.
    pub(super) fn close_tab(&self, key: &str) -> Result<String, String> {
        let tab = parse(key).ok_or_else(|| format!("`{key}` is not a tab id"))?;
        let owner = {
            let mut registry = crate::lock(&self.registry);
            let owner = registry.owner_named(&tab.key);
            registry.release(&tab.key);
            owner
        };
        if owner.is_some() {
            self.gone(owner, &tab.key);
        }
        match tab.kind {
            Kind::Web => self.close_web(tab.id),
            Kind::Term => self.close_term(tab.id),
            Kind::Board => self.close_board(&tab.key),
        }
    }

    /// The owner's tabs, one per line.
    fn list(&self, owner: &str) -> String {
        self.prune_pages();
        let keys = crate::lock(&self.registry).tabs_of(owner);
        if keys.is_empty() {
            return "No tabs open.".into();
        }
        let window = self
            .browser
            .as_ref()
            .and_then(crate::browser::Browser::listing)
            .unwrap_or_default();
        let mut out = String::new();
        for key in keys {
            let Some(tab) = parse(&key) else { continue };
            let known = window.iter().find(|w| w.id == key);
            let detail = match (tab.kind, known) {
                (Kind::Board, _) => self.board_summary(&tab.key),
                (_, Some(w)) => format!("{} {}", w.title, w.detail).trim().to_string(),
                (Kind::Term, None) => self
                    .session(tab.id)
                    .map(|s| s.command().to_string())
                    .unwrap_or_default(),
                (Kind::Web, None) => String::new(),
            };
            let shown = if known.is_some_and(|w| w.shown) {
                " [shown]"
            } else {
                ""
            };
            let _ = writeln!(out, "{key} ({}){shown}: {detail}", tab.kind.name());
        }
        out.trim_end().to_string()
    }
}
