//! The `window` tool's actions: open, interact, screenshot, close, show, list and resize, over
//! the owner's tabs.

use std::fmt::Write as _;

use serde_json::{Value, json};

use super::args::{TERM_FIELDS, WEB_FIELDS, number, picked, strings, text};
use super::{Call, Desk, Failure, Output};

/// What a tab is, from its key.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Web,
    Term,
}

impl Kind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Web => "browser",
            Self::Term => "terminal",
        }
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Web => &WEB_FIELDS,
            Self::Term => &TERM_FIELDS,
        }
    }
}

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
                })
            }
            Some("close") => self.close(call, args),
            Some("show") => {
                let tab = self.tab(call, text(args, "tab"))?;
                let browser = self
                    .browser
                    .as_ref()
                    .ok_or_else(|| "there is no window here to show it in".to_string())?;
                browser.act(&tab.key, false)?;
                let read = self.read(&tab)?;
                Ok(Output {
                    report: format!("Showing {}.", tab.key),
                    ..read
                })
            }
            Some("list") => Ok(self.list(call.owner).into()),
            Some("resize") => {
                let browser = self
                    .browser
                    .as_ref()
                    .ok_or_else(|| "there is no window here to size".to_string())?;
                let (Some(width), Some(height)) = (number(args, "width"), number(args, "height"))
                else {
                    return Err("`width` and `height` are needed, in points"
                        .to_string()
                        .into());
                };
                let points = |n: u64| f64::from(u32::try_from(n).unwrap_or(u32::MAX));
                browser.resize_window(points(width), points(height))?;
                Ok(format!("The window is {width}x{height} now.").into())
            }
            _ => Err(
                "`action` is one of open, interact, screenshot, close, show, list, resize"
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
            _ => Err(format!(
                "there is no tab {} among yours: `list` names them",
                tab.key
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
        }
    }

    /// Acting in a tab, a page answering its dialogs as the call asks; what the page told
    /// during the call after the report.
    fn interact(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        let tab = self.tab(call, text(args, "tab"))?;
        if tab.kind == Kind::Term {
            return self.interact_in(call, &tab, args);
        }
        let mark = self.told_mark();
        let answering = self.answering(tab.id, args)?;
        let mut done = self.interact_in(call, &tab, args)?;
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

    /// Acting in a tab: a goal for the helper first, then the kind's own fields.
    fn interact_in(&self, call: &Call, tab: &Tab, args: &Value) -> Result<Output, Failure> {
        let Some(goal) = text(args, "goal") else {
            let mut done = self.interact_kind(call, tab, args)?;
            done.report = format!("{}: {}", tab.key, done.report);
            return Ok(done);
        };
        // The kind's own fields first (an address to load), so the goal acts on what they leave.
        let own = tab
            .kind
            .fields()
            .iter()
            .any(|field| args.get(*field).is_some());
        let first = if own {
            Some(self.interact_kind(call, tab, args)?)
        } else {
            None
        };
        let helper = self.goal_on(call, tab, goal, &strings(args, "values"))?;
        let mut after = self.interact_kind(call, tab, &json!({}))?;
        let mut report = vec![];
        if let Some(first) = first.filter(|f| !f.report.is_empty()) {
            report.push(format!("{}: {}", tab.key, first.report));
        }
        report.push(helper);
        if !after.report.is_empty() {
            report.push(std::mem::take(&mut after.report));
        }
        after.report = report.join("\n\n");
        Ok(after)
    }

    fn interact_kind(&self, call: &Call, tab: &Tab, args: &Value) -> Result<Output, String> {
        match tab.kind {
            Kind::Web => self.interact_web(call, tab.id, args),
            Kind::Term => self.interact_term(tab.id, &picked(args, &TERM_FIELDS)),
        }
    }

    /// `goal` on `tab`'s view, while no other call works in it.
    pub(super) fn goal_on(
        &self,
        call: &Call,
        tab: &Tab,
        goal: &str,
        values: &[String],
    ) -> Result<String, Failure> {
        match tab.kind {
            Kind::Web => {
                let lock = self.tab_lock(tab.id);
                let _held = crate::lock(&lock);
                let page = self.page()?.pinned(tab.id);
                let mut paid = crate::lock(&self.paid).remove(&tab.id).unwrap_or_default();
                let done = super::goal::run_goal(
                    call,
                    &tab.key,
                    &page,
                    goal,
                    values,
                    &self.vault,
                    &mut paid,
                );
                crate::lock(&self.paid).insert(tab.id, paid);
                done
            }
            Kind::Term => {
                let session = self.session(tab.id)?;
                super::goal::run_goal(
                    call,
                    &tab.key,
                    &crate::terminal::TerminalView(&session),
                    goal,
                    values,
                    &self.vault,
                    &mut vec![],
                )
            }
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
        if let Some(owner) = owner {
            self.events
                .publish("tab/closed", Some(&owner), json!({"tab": tab.key}));
        }
        match tab.kind {
            Kind::Web => self.close_web(tab.id),
            Kind::Term => self.close_term(tab.id),
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
