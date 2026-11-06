//! A terminal's tab: starting a program in a session, typing into it, reading its screen.

use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock, Weak};
use std::time::Duration;

use serde_json::Value;

use crate::browser::Pane;
use crate::terminal::{self, DEFAULT_WAIT, Painter, Session, Until};

use super::args::{number, strings, text};
use super::ledger::Facts;
use super::{Call, Desk, Output};

/// Terminal sessions at once, in all.
const MAX_SESSIONS: usize = 16;
/// The most text one `input` types.
const MAX_INPUT: usize = 4096;

impl Desk {
    /// Start `args["command"]` (the person's shell without one) in a new session. The id is
    /// returned when the program is still running after the wait; one that ended is not kept.
    pub(super) fn open_term(
        &self,
        call: &Call,
        args: &Value,
    ) -> Result<(Option<u32>, Output), String> {
        let dir = term_dir(call, args);
        if !dir.is_dir() {
            return Err(format!("{} is not a directory", dir.display()));
        }
        let group = crate::projects::key(call.project.id);
        let (id, session) = self.start_session(
            text(args, "command").unwrap_or(""),
            &dir,
            call.vars,
            &group,
            "",
        )?;
        let wanted = text(args, "wait_for");
        let most = number(args, "yield_ms").map_or(DEFAULT_WAIT, Duration::from_millis);
        let until = session.wait(wanted, 0, most);
        let output = Output {
            report: describe(until, wanted, most),
            state: Some(session.snapshot()),
            shot: None,
        };
        if until == Until::Exited {
            self.drop_session(id);
            return Ok((None, output));
        }
        Ok((Some(id), output))
    }

    /// A session in a tab of the window (where there is one), its id and handle.
    pub(super) fn start_session(
        &self,
        command: &str,
        dir: &std::path::Path,
        vars: Option<&[(String, String)]>,
        group: &str,
        before: &str,
    ) -> Result<(u32, Arc<Session>), String> {
        let _starting = crate::lock(&self.starting);
        if crate::lock(&self.sessions).len() >= MAX_SESSIONS {
            let spare = self
                .spare_session(group)
                .ok_or_else(|| format!("{MAX_SESSIONS} terminals are open: close one first"))?;
            // Its tab goes as any closed one: no longer anyone's, closed in the ledger.
            let key = format!("t{spare}");
            let owner = {
                let mut registry = crate::lock(&self.registry);
                let owner = registry.owner_named(&key);
                registry.release(&key);
                owner
            };
            if owner.is_some() {
                self.gone(owner, &key);
            }
            self.drop_session(spare);
        }
        let id = self.next_terminal.fetch_add(1, Ordering::SeqCst);
        let typed: Arc<OnceLock<Weak<Session>>> = Arc::default();
        let painter = self.painter(id, command, group, &typed);
        let session = Session::start_after(command, dir, vars, painter, before)
            .map_err(|e| format!("could not start it: {e}"))?;
        let session = Arc::new(session);
        let _ = typed.set(Arc::downgrade(&session));
        // Its program's end is news for the Event Center.
        // Its program's end closes its row of the ledger (its last screen kept), on a thread of
        // its own: the session's reader calls this.
        let (feed, me) = (self.feed.clone(), Weak::clone(&self.me));
        session.on_end(move || {
            feed.record(
                &crate::feed::Entity::Terminal { id },
                crate::feed::Op::Upserted,
            );
            let me = Weak::clone(&me);
            let closed = std::thread::Builder::new()
                .name("desk-terminal-ended".into())
                .spawn(move || {
                    if let Some(desk) = me.upgrade() {
                        desk.unkeep(&format!("t{id}"));
                    }
                });
            if let Err(why) = closed {
                log::warn!("terminal t{id} ended, still open in the ledger (no thread: {why})");
            }
        });
        // What its program says it is doing (the title it sets) shows under the tab's name, and
        // which program is in front (a coding agent typed in its shell among them) is the window's
        // to know: its mark on the tab, its integration offered.
        // Where it is (the bar's crumbs) is asked again as its shell prompts (it sets the title)
        // and as another program comes in front.
        if let Some(browser) = self.browser.clone() {
            let (told, me) = (browser.clone(), Weak::clone(&self.me));
            let (here, there) = (Arc::downgrade(&session), Arc::downgrade(&session));
            browser.folder(
                &format!("t{id}"),
                session.folder().or_else(|| Some(dir.into())),
            );
            session.on_title(move |title| {
                told.detail(&format!("t{id}"), Some(title));
                if let Some(session) = here.upgrade() {
                    told.folder(&format!("t{id}"), session.folder());
                }
                if let Some(desk) = me.upgrade() {
                    desk.retitled(&format!("t{id}"), title);
                }
            });
            session.on_front(move |program| {
                browser.runs(&format!("t{id}"), program);
                if let Some(session) = there.upgrade() {
                    browser.folder(&format!("t{id}"), session.folder());
                }
            });
        }
        crate::lock(&self.sessions).insert(id, Arc::clone(&session));
        Ok((id, session))
    }

    /// The tab that shows session `id`, where a window shows; what the person types in it goes
    /// to the session once `typed` holds it.
    fn painter(
        &self,
        id: u32,
        command: &str,
        group: &str,
        typed: &Arc<OnceLock<Weak<Session>>>,
    ) -> Option<Painter> {
        let browser = self.browser.as_ref()?;
        let shown = if command.trim().is_empty() {
            "shell"
        } else {
            command.trim()
        };
        let title = format!("desk · session {id} · {shown}");
        browser.group(&format!("t{id}"), group);
        let panel = browser.panel(id, &title, terminal::page(&title));
        let typed_to = Arc::clone(typed);
        let typed = Arc::clone(typed);
        panel.keyboard(Arc::new(move |keys: &str| {
            if let Some(session) = typed.get().and_then(Weak::upgrade) {
                session.typed(keys);
            }
        }));
        let sized = Arc::clone(&typed_to);
        panel.sizing(Arc::new(move |pane, columns, rows| {
            if let Some(session) = sized.get().and_then(Weak::upgrade) {
                match pane {
                    Pane::Loaded => session.loaded(columns, rows),
                    Pane::Resized => session.resized(columns, rows),
                }
            }
        }));
        Some(Box::new(move |frame| panel.paint(&frame)))
    }

    /// The session of project `group` to end for a new one of it when all places are taken: the
    /// oldest whose program ended, else the oldest whose tab the person closed (its program
    /// running out of sight). None while every one is live in the window: another project's
    /// terminals are never ended for this one's.
    fn spare_session(&self, group: &str) -> Option<u32> {
        let of_group = |id: u32| {
            (crate::lock(&self.ledger).get(&format!("t{id}")))
                .is_some_and(|(_, project)| crate::projects::key(*project) == group)
        };
        let mut ids: Vec<(u32, bool)> = crate::lock(&self.sessions)
            .iter()
            .map(|(id, session)| (*id, session.ended()))
            .collect();
        ids.retain(|(id, _)| of_group(*id));
        ids.sort_unstable();
        if let Some((id, _)) = ids.iter().find(|(_, ended)| *ended) {
            return Some(*id);
        }
        let shown = self.browser.as_ref()?.listing()?;
        ids.iter()
            .map(|(id, _)| *id)
            .find(|id| !shown.iter().any(|tab| tab.id == format!("t{id}")))
    }

    fn drop_session(&self, id: u32) {
        let session = crate::lock(&self.sessions).remove(&id);
        if let Some(session) = session {
            session.close();
        }
        if let Some(browser) = &self.browser {
            browser.detail(&format!("t{id}"), None);
        }
    }

    pub(super) fn session(&self, id: u32) -> Result<Arc<Session>, String> {
        crate::lock(&self.sessions)
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("there is no terminal t{id}: it was closed"))
    }

    /// Type into session `id` and read what it shows.
    pub(super) fn interact_term(&self, id: u32, args: &Value) -> Result<Output, String> {
        let session = self.session(id)?;
        let given = args.get("input").and_then(Value::as_str).unwrap_or("");
        // A secret it names is put in here, and shows as its name in every answer after.
        let input = &self
            .vault
            .fill_in(crate::secrets::dir().as_deref(), given)?;
        if input.len() > MAX_INPUT {
            return Err(format!(
                "`input` is at most {MAX_INPUT} bytes: type it in parts"
            ));
        }
        let wanted = text(args, "wait_for");
        // What the input itself holds the terminal echoes: the wait is for the program's output.
        let before = wanted.map_or(0, |wanted| {
            session.count(wanted)
                + if wanted.is_empty() {
                    0
                } else {
                    input.matches(wanted).count()
                }
        });
        let keys = strings(args, "keys");
        if !input.is_empty() || !keys.is_empty() {
            session.type_in(input, &keys)?;
        }
        let most = number(args, "yield_ms").map_or(DEFAULT_WAIT, Duration::from_millis);
        let until = session.wait(wanted, before, most);
        Ok(Output {
            report: describe(until, wanted, most),
            state: Some(session.snapshot()),
            shot: None,
        })
    }

    pub(super) fn close_term(&self, id: u32) -> Result<String, String> {
        if !crate::lock(&self.sessions).contains_key(&id) {
            return Err(format!("there is no terminal t{id}"));
        }
        self.drop_session(id);
        Ok(format!("Closed t{id}."))
    }

    /// The person asked for a terminal of their own ("+") in the project grouped as `group`.
    /// A terminal the person opened with "+", in the project's folder: their shell, or `command`
    /// (a coding agent's program) when it is one.
    pub(super) fn person_terminal(
        &self,
        call: &Call,
        group: &str,
        command: &str,
    ) -> Result<(), String> {
        self.person_terminal_after(call, group, command, "")
    }

    /// `person_terminal`, its screen showing `before` first, dimmed: a terminal restored.
    pub(super) fn person_terminal_after(
        &self,
        call: &Call,
        group: &str,
        command: &str,
        before: &str,
    ) -> Result<(), String> {
        let (id, _) = self.start_session(command, call.cwd, call.vars, group, before)?;
        let facts = Facts {
            title: command,
            cwd: Some(call.cwd),
            command: Some(command).filter(|command| !command.is_empty()),
            ..Facts::default()
        };
        self.own(
            &format!("t{id}"),
            call,
            crate::ledger::Kind::Terminal,
            facts,
        );
        Ok(())
    }
}

/// Where a terminal `args` asks for starts: its `workdir`, from the caller's directory.
pub(super) fn term_dir(call: &Call, args: &Value) -> std::path::PathBuf {
    match text(args, "workdir") {
        Some(dir) => call.cwd.join(dir),
        None => call.cwd.to_path_buf(),
    }
}

/// How a wait ended, in words.
fn describe(until: Until, wanted: Option<&str>, most: Duration) -> String {
    match (until, wanted) {
        (Until::Matched, Some(wanted)) => format!("\"{wanted}\" showed."),
        (Until::Matched, None) | (Until::Settled, _) => "The output settled.".into(),
        (Until::Deadline, Some(wanted)) => format!(
            "\"{wanted}\" did not show within {}s: the program may still be working; wait again or read the screen.",
            most.as_secs()
        ),
        (Until::Deadline, None) => format!("Still changing after {}s.", most.as_secs()),
        (Until::Exited, _) => "The program ended.".into(),
    }
}
