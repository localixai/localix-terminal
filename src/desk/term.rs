//! A terminal's tab: starting a program in a session, typing into it, reading its screen.

use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock, Weak};
use std::time::Duration;

use serde_json::Value;

use crate::browser::Pane;
use crate::terminal::{self, DEFAULT_WAIT, Painter, Session, Until};

use super::args::{number, strings, text};
use super::{Call, Desk, Output};

/// Terminal sessions at once, in all.
const MAX_SESSIONS: usize = 8;
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
        let dir = match text(args, "workdir") {
            Some(dir) => call.cwd.join(dir),
            None => call.cwd.to_path_buf(),
        };
        if !dir.is_dir() {
            return Err(format!("{} is not a directory", dir.display()));
        }
        let group = crate::lock(&self.registry).group_of(call.owner);
        let (id, session) =
            self.start_session(text(args, "command").unwrap_or(""), &dir, call.vars, &group)?;
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
    ) -> Result<(u32, Arc<Session>), String> {
        if crate::lock(&self.sessions).len() >= MAX_SESSIONS {
            return Err(format!(
                "{MAX_SESSIONS} terminals are open: close one first"
            ));
        }
        let id = self.next_terminal.fetch_add(1, Ordering::SeqCst);
        let typed: Arc<OnceLock<Weak<Session>>> = Arc::default();
        let painter = self.painter(id, command, group, &typed);
        let session = Session::start(command, dir, vars, painter)
            .map_err(|e| format!("could not start it: {e}"))?;
        let session = Arc::new(session);
        let _ = typed.set(Arc::downgrade(&session));
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
        let panel = browser.panel(id, &title, terminal::page(&title, None), None);
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

    fn drop_session(&self, id: u32) {
        let session = crate::lock(&self.sessions).remove(&id);
        if let Some(session) = session {
            session.close();
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
        let session = crate::lock(&self.sessions)
            .remove(&id)
            .ok_or_else(|| format!("there is no terminal t{id}"))?;
        session.close();
        Ok(format!("Closed t{id}."))
    }

    /// The person asked for a terminal of their own ("+") in the project grouped as `group`.
    pub(super) fn person_terminal(self: &Arc<Self>, group: &str) {
        let Some((cwd, vars, owner)) = crate::lock(&self.registry).of_group(group) else {
            return;
        };
        let (desk, group) = (Arc::clone(self), group.to_string());
        // On a thread of its own: the window's loop calls this and must never wait on a program.
        let started = std::thread::Builder::new()
            .name("desk-person-shell".into())
            .spawn(
                move || match desk.start_session("", &cwd, vars.as_deref(), &group) {
                    Ok((id, _)) => crate::lock(&desk.registry).own(&format!("t{id}"), &owner),
                    Err(why) => log::warn!("a terminal the person asked for did not start: {why}"),
                },
            );
        if let Err(why) = started {
            log::warn!("a terminal the person asked for had no thread: {why}");
        }
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
