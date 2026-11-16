//! The `window` tool's actions: open, interact, screenshot, close, show and list, over
//! the owner's tabs.

use serde_json::{Value, json};

use super::args::{TERM_FIELDS, picked, text};
use super::center::Subject;
use super::tab::unread;
pub(super) use super::tab::{Kind, Tab, parse};
use super::{Call, Desk, Failure, Output};

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
                        missed: false,
                    },
                    Kind::Board => Output {
                        report: "A board is read as text: it is below.".into(),
                        state: Some(self.board_text(&tab.key)?),
                        shot: None,
                        missed: false,
                    },
                    Kind::File => Output {
                        report: "A file is read as text: it is below.".into(),
                        state: Some(self.file_text(&tab.key)?),
                        shot: None,
                        missed: false,
                    },
                    Kind::Made(about) => Output {
                        report: "It is read as text, below.".into(),
                        state: Some(self.made_text(about)),
                        shot: None,
                        missed: false,
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
                // So is a tab of a kind that draws itself.
                if let Kind::Made(about) = tab.kind {
                    return self.made(call, about, &json!({}));
                }
                self.reveal(call, &tab.key)?;
                // Its tab zoomed on it, or drawn whole again (layout_zoom.rs).
                let zoomed = match args.get("zoom").and_then(Value::as_bool) {
                    Some(on) => format!(" {}", self.zoom_shown(&tab.key, on)?),
                    None => String::new(),
                };
                let read = self.read(&tab)?;
                Ok(Output {
                    report: format!("Showing {}.{zoomed}", tab.key),
                    ..read
                })
            }
            Some("list") => Ok(self.list(call.owner).into()),
            Some("steps") => self.steps(call, args),
            // The guide, for a caller that has no shell to print it with.
            Some("guide") => {
                let kinds: Vec<_> = self.made.kinds.iter().map(|kind| kind.about()).collect();
                Ok(super::args::guide(text(args, "topic"), &kinds)?.into())
            }
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
                "`action` is one of open, interact, screenshot, close, show, list, steps, guide"
                    .to_string()
                    .into(),
            ),
        }
    }

    /// The owner's tab named `key`.
    pub(super) fn tab(&self, call: &Call, key: Option<&str>) -> Result<Tab, String> {
        let key = key.ok_or("`tab` is needed: an id from `open` or `list`")?;
        let tab = parse(key, &self.made.kinds)
            .ok_or_else(|| format!("`{key}` is not a tab id: they look like w1, t2"))?;
        if tab.kind == Kind::Web {
            self.prune_pages();
        }
        let owner = crate::lock(&self.registry).owner_named(&tab.key);
        let found = match owner {
            Some(owner) if owner == call.owner => Ok(tab),
            // The project's board is one tab for all its agents: whoever opened it first owns it.
            _ if tab.kind == Kind::Board && self.board_of(call.project.id, &tab.key) => Ok(tab),
            // A file has one tab in a project too, whoever opened it.
            _ if tab.kind == Kind::File && self.file_of(call.project.id, &tab.key) => Ok(tab),
            // And so has a kind that draws itself.
            _ if self.made_of(call.project.id, &tab) => Ok(tab),
            // A call from another directory is another owner's: say whose, so the agent sees it
            // left its project (a `cd` into a scratch directory) rather than lost the tab.
            _ => Err(format!(
                "there is no tab {} among yours, {}: `list` names them. Tabs are kept per \
                 project, the directory with `.git` around where the call runs (or \
                 LLXTERM_PROJECT, or `--as`): run it from the project's directory",
                tab.key, call.owner
            )),
        };
        // Where a later step that names no tab works.
        if let Ok(tab) = &found {
            crate::lock(&self.registry).used(&tab.key);
        }
        found
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
            Kind::File => Ok(Output {
                report: self.file_summary(&tab.key),
                state: Some(self.file_text(&tab.key)?),
                shot: None,
                missed: false,
            }),
            Kind::Made(about) => Ok(Output {
                state: Some(self.made_text(about)),
                ..Output::default()
            }),
        }
    }

    /// Acting in a tab, a page answering its dialogs as the call asks; what the page told
    /// during the call after the report.
    fn interact(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        // An ask with no tab: the session waits for the person.
        if text(args, "tab").is_none() && args.get("ask").is_some() {
            return Ok(self.waits(call, text(args, "ask")).into());
        }
        // A coding agent's own hooks name the session its terminal runs.
        if let (None, Some(session)) = (text(args, "tab"), args.get("session")) {
            return Ok(self
                .agent_said(call, session.as_str().unwrap_or_default())?
                .into());
        }
        let tab = self.tab(call, text(args, "tab"))?;
        unread(tab.kind, args)?;
        // An ask of the person, on any kind of tab, and then what the call does there.
        let asked = (args.get("ask")).map(|_| {
            let on = Subject::Tab(tab.key.clone());
            self.ask(call.owner, on, text(args, "ask"))
        });
        let mut done = match self.interact_tab(call, &tab, args) {
            Ok(done) => done,
            Err(failure) => {
                // The ask rode on an act that did not happen: the person is not asked for it.
                if text(args, "ask").is_some() {
                    self.ask(call.owner, Subject::Tab(tab.key.clone()), None);
                }
                return Err(failure);
            }
        };
        if let Some(asked) = asked {
            done.report = format!("{asked}\n\n{}", done.report);
        }
        Ok(done)
    }

    fn interact_tab(&self, call: &Call, tab: &Tab, args: &Value) -> Result<Output, Failure> {
        let mut done = match tab.kind {
            Kind::Term => self.interact_term(tab.id, &picked(args, &TERM_FIELDS))?,
            // A board's `op` acts on it; with none, it is read (what a page or a terminal is
            // acted on with was refused already).
            Kind::Board => return self.board(call, args),
            // A file's tab is read: every field that would act on it was refused already.
            Kind::File => self.read(tab)?,
            // What a kind that draws itself is told is its own to answer.
            Kind::Made(about) => return self.made(call, about, args),
            Kind::Web => {
                let mark = self.told_mark();
                let answering = self.answering(tab.id, args)?;
                let mut done = self.interact_web(call, tab.id, args)?;
                drop(answering);
                if let Some(told) = self.told(tab.id, mark) {
                    done.report = format!("{}\n\n{told}", done.report);
                }
                done
            }
        };
        done.report = match done.report.is_empty() {
            true => format!("{}:", tab.key),
            false => format!("{}: {}", tab.key, done.report),
        };
        Ok(done)
    }
}
