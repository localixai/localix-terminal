//! A terminal's tab: starting a program in a session, typing into it, reading its screen.

mod env;
mod listen;
mod room;

pub(super) use room::{Room, are_open};

use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock, Weak};

use serde_json::Value;

use crate::browser::Pane;
use crate::feed::{Entity, Op};
use crate::terminal::{self, DEFAULT_WAIT, Painter, Session, Until};

use super::args::{strings, text, waits};
use super::ledger::Facts;
use super::{Call, Desk, Output};

/// The most text one `input` types, and the most keys one call presses.
const MAX_INPUT: usize = 4096;
const MAX_KEYS: usize = 64;

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
        // Opened from a terminal, it is a session of its own and leaves the window to that one.
        let aside = self.quiet(call);
        let (id, session) = self.start_session(
            text(args, "command").unwrap_or(""),
            &dir,
            call.vars,
            (&group, aside),
            "",
        )?;
        let wanted = text(args, "wait_for");
        let most = waits(args, DEFAULT_WAIT);
        let until = session.wait(wanted, 0, most);
        let output = Output {
            report: until.said(wanted, most),
            state: Some(session.snapshot()),
            shot: None,
            missed: missed(until, wanted),
        };
        if until == Until::Exited {
            self.drop_session(id);
            return Ok((None, output));
        }
        Ok((Some(id), output))
    }

    /// A session in a tab of the window (where there is one), its id and handle. The tab is of
    /// project `group`; `aside`, it opens beside the tab in sight and does not take the window.
    pub(super) fn start_session(
        &self,
        command: &str,
        dir: &std::path::Path,
        vars: Option<&[(String, String)]>,
        (group, aside): (&str, bool),
        before: &str,
    ) -> Result<(u32, Arc<Session>), String> {
        let _starting = crate::lock(&self.starting);
        let open = crate::lock(&self.sessions).len();
        if open >= self.room.most() {
            // Room is made only of a terminal whose program ended: a program that runs is never
            // ended for another, whatever became of its tab.
            let Some(ended) = self.ended_session(group) else {
                let refused = self.room.refuse(group, command, open);
                self.feed.record(&Entity::Room, Op::Upserted);
                return Err(refused);
            };
            // Its tab goes as any closed one: no longer anyone's, closed in the ledger.
            self.let_go(&format!("t{ended}"));
            self.drop_session(ended);
        }
        let id = self.next_terminal.fetch_add(1, Ordering::SeqCst);
        let typed: Arc<OnceLock<Weak<Session>>> = Arc::default();
        let painter = self.painter(id, command, (group, aside), &typed);
        let folder = crate::projects::of_key(group)
            .and_then(|project| self.project_by_id(project))
            .map(|project| project.path);
        let own = env::own(id, folder.as_deref(), vars);
        let session = Session::start_after(command, dir, vars, painter, before, &own)
            .map_err(|e| format!("could not start it: {e}"))?;
        let session = Arc::new(session);
        let _ = typed.set(Arc::downgrade(&session));
        self.hear_session(id, &session, dir);
        crate::lock(&self.session_groups).insert(id, group.to_string());
        crate::lock(&self.sessions).insert(id, Arc::clone(&session));
        if self.room.opened(group) {
            self.feed.record(&Entity::Room, Op::Upserted);
        }
        Ok((id, session))
    }

    /// The tab that shows session `id`, where a window shows; what the person types in it goes
    /// to the session once `typed` holds it.
    fn painter(
        &self,
        id: u32,
        command: &str,
        (group, aside): (&str, bool),
        typed: &Arc<OnceLock<Weak<Session>>>,
    ) -> Option<Painter> {
        let browser = self.browser.as_ref()?;
        let shown = if command.trim().is_empty() {
            "shell"
        } else {
            command.trim()
        };
        // Named by what it runs: its key (`t3`) already says which terminal it is.
        let title = format!("desk · {shown}");
        browser.group(&format!("t{id}"), group);
        if aside {
            browser.aside(&format!("t{id}"));
        }
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
        Some(Box::new(move |frame| {
            let open = panel.paint(&frame);
            // Output the panel had to drop leaves its page behind the screen: it is sent the
            // screen whole.
            if panel.dropped()
                && let Some(session) = typed_to.get().and_then(Weak::upgrade)
            {
                session.behind();
            }
            open
        }))
    }

    /// The session of project `group` to let go of for a new one of it when the room is full:
    /// the oldest whose program ended. None while every one runs; another project's terminals
    /// are never let go of for this one's.
    fn ended_session(&self, group: &str) -> Option<u32> {
        let groups = crate::lock(&self.session_groups).clone();
        let sessions = crate::lock(&self.sessions);
        (sessions.iter())
            .filter(|(id, session)| {
                session.ended() && groups.get(id).is_some_and(|started| started == group)
            })
            .map(|(id, _)| *id)
            .min()
    }

    fn drop_session(&self, id: u32) {
        let session = crate::lock(&self.sessions).remove(&id);
        crate::lock(&self.session_groups).remove(&id);
        if let Some(session) = session {
            session.close();
        }
        if let Some(browser) = &self.browser {
            browser.detail(&format!("t{id}"), None);
            browser.working(&format!("t{id}"), None);
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
        let keys = strings(args, "keys");
        // What the input itself holds the terminal echoes: the wait is for the program's output.
        // With nothing to type, it is for the answer to what was typed last.
        let typing = !input.is_empty() || !keys.is_empty();
        let before = wanted.map_or(0, |wanted| match typing {
            true => session.count(wanted) + input.matches(wanted).count(),
            false => session.count_before_typed(wanted),
        });
        if keys.len() > MAX_KEYS {
            return Err(format!("`keys` is at most {MAX_KEYS}: press them in parts"));
        }
        if !input.is_empty() || !keys.is_empty() {
            session.type_in(input, &keys)?;
        }
        let most = waits(args, DEFAULT_WAIT);
        let until = session.wait(wanted, before, most);
        Ok(Output {
            report: until.said(wanted, most),
            state: Some(session.snapshot()),
            shot: None,
            missed: missed(until, wanted),
        })
    }

    pub(super) fn close_term(&self, id: u32) -> Result<String, String> {
        if !crate::lock(&self.sessions).contains_key(&id) {
            return Err(format!("there is no terminal t{id}"));
        }
        self.drop_session(id);
        Ok(format!("Closed t{id}."))
    }

    /// A terminal the person asked for with "+" in the project grouped as `group`, in the
    /// project's folder: their shell, or `command` (a coding agent's program) when it is one.
    pub(super) fn person_terminal(
        &self,
        call: &Call,
        group: &str,
        command: &str,
    ) -> Result<(), String> {
        self.person_terminal_after(call, group, command, "")
    }

    /// `person_terminal`, titled `title` rather than by its command line (a long one).
    pub(super) fn person_terminal_titled(
        &self,
        call: &Call,
        group: &str,
        command: &str,
        title: &str,
    ) -> Result<(), String> {
        (self.terminal_of_person(call, group, command, title, "")).map(drop)
    }

    /// `person_terminal`, its screen showing `before` first, dimmed: a terminal restored.
    pub(super) fn person_terminal_after(
        &self,
        call: &Call,
        group: &str,
        command: &str,
        before: &str,
    ) -> Result<(), String> {
        (self.terminal_of_person(call, group, command, command, before)).map(drop)
    }

    /// The person's terminal, as each way of asking for one starts it: its tab's key.
    pub(super) fn terminal_of_person(
        &self,
        call: &Call,
        group: &str,
        command: &str,
        title: &str,
        before: &str,
    ) -> Result<String, String> {
        self.terminal_placed(call, group, (command, title, before), None)
    }

    /// `terminal_of_person`, put on a side of a pane (`split`: its row and the side) in that
    /// pane's tab rather than a tab of its own: opened aside, and shown once it is there.
    pub(super) fn terminal_placed(
        &self,
        call: &Call,
        group: &str,
        (command, title, before): (&str, &str, &str),
        split: Option<(crate::ledger::Row, crate::layout::geometry::Side)>,
    ) -> Result<String, String> {
        let aside = split.is_some();
        // An agent's own command, as "+" starts it, names the new session itself: its terminal
        // knows which one it runs from the start.
        let pinned = crate::browser::agent_pinned(command);
        let command = pinned.as_deref().unwrap_or(command);
        let (id, _) = self.start_session(command, call.cwd, call.vars, (group, aside), before)?;
        let facts = Facts {
            title,
            cwd: Some(call.cwd),
            command: Some(command).filter(|command| !command.is_empty()),
            split,
            ..Facts::default()
        };
        let key = format!("t{id}");
        self.own(&key, call, crate::ledger::Kind::Terminal, facts);
        // Its agent, if one is in front already, is kept now that its pane has a row.
        self.agent_fronted(id);
        // Put beside a pane, it is in sight once its tab holds it: the person asked for it, so
        // it takes the keys as their own click on it would.
        if let (Some(_), Some(browser)) = (split, &self.browser) {
            browser.click(&format!("show {key}"));
        }
        Ok(key)
    }
}

/// A wait for text that ended any other way than with the text: it did not hold.
fn missed(until: Until, wanted: Option<&str>) -> bool {
    wanted.is_some() && until != Until::Matched
}

/// Where a terminal `args` asks for starts: its `workdir`, from the caller's directory.
pub(super) fn term_dir(call: &Call, args: &Value) -> std::path::PathBuf {
    match text(args, "workdir") {
        Some(dir) => call.cwd.join(dir),
        None => call.cwd.to_path_buf(),
    }
}

#[cfg(test)]
#[path = "term_tests.rs"]
mod tests;
