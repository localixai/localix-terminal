//! Window events adapted into desk actions, with disk writes kept off the UI thread.

use std::sync::Arc;

use serde_json::json;

use crate::browser::{Asked, HOME, Heard, Went};

use super::{Call, Desk};

impl Desk {
    /// What the person did in the window: a comment, a closed tab, a terminal asked for.
    pub(super) fn heard(self: &Arc<Self>, key: &str, heard: Heard<'_>) {
        match heard {
            Heard::Sent(words) | Heard::Did(words) => self.kept(key, words),
            // A tab the person closed is no news worth waking the agent for: it hears it with its
            // next call; one the person opened themselves is no news to an agent at all. A board
            // is kept; a page is gone and let go; a terminal is gone, its session still the
            // desk's (its owner and project are what make room for a new one).
            Heard::Closed(words) => {
                let person = crate::lock(&self.registry).persons(key);
                if !person {
                    self.hear(key, words, false);
                }
                if key.starts_with('b') {
                    return self.closed_later(key, None, false);
                }
                let owner = self.owner_of(key).filter(|_| !person);
                if let Some(id) = key.strip_prefix('w').and_then(|n| n.parse::<u32>().ok()) {
                    crate::lock(&self.registry).release(key);
                    self.forget_page(id, 1);
                }
                self.closed_later(key, owner, true);
            }
            Heard::Went(went) => {
                let (desk, key) = (Arc::clone(self), key.to_string());
                let (url, title, group) = (
                    went.url.to_string(),
                    went.title.to_string(),
                    went.group.to_string(),
                );
                self.write_later(move || {
                    let went = Went {
                        url: &url,
                        title: &title,
                        group: &group,
                    };
                    desk.went(&key, &went);
                });
            }
            Heard::Folder(folder) => self.open_folder(folder),
            Heard::Showing => {
                let (desk, group, window) = (Arc::clone(self), key.to_string(), self.in_use());
                self.write_later(move || desk.showing(&group, window));
            }
            Heard::Split(halves) => {
                if let Some(split) = self.split_of(halves, false) {
                    let desk = Arc::clone(self);
                    self.write_later(move || desk.split_kept(split));
                }
            }
            Heard::Whole(halves) => {
                if let Some(split) = self.split_of(halves, true) {
                    let desk = Arc::clone(self);
                    self.write_later(move || desk.split_kept(split));
                }
            }
            Heard::SplitResized(portion) => {
                let key = key.to_string();
                let desk = Arc::clone(self);
                self.write_later(move || {
                    if let Err(why) = desk.split_resized(&key, portion) {
                        log::warn!("the split size was not kept: {why}");
                    }
                });
            }
            // The panes are found by their rows as the window tells it; the layout is written
            // after what the window told before.
            Heard::Moved { before, window } => {
                if let Some(rows) = self.rows_moved(key, before) {
                    let (desk, key) = (Arc::clone(self), key.to_string());
                    self.write_later(move || desk.move_kept(&key, rows, window));
                }
            }
            Heard::Detached => {
                if let Some((pane, _)) = self.rows_moved(key, None) {
                    let (desk, key) = (Arc::clone(self), key.to_string());
                    self.write_later(move || desk.detach_kept(&key, pane));
                }
            }
            Heard::WindowClosed(window) => {
                let desk = Arc::clone(self);
                self.write_later(move || desk.window_closed(window));
            }
            Heard::Quit => self.quit_asked(),
            Heard::Reopened => self.reopened(),
            Heard::Asked(asked) => self.person_asked(key, asked),
            Heard::Center(did) => self.person_on_center(did),
        }
    }

    /// How long what the person asks for waits for the writes the window told before it: a tab
    /// they closed a moment ago is closed in the ledger before they open it again.
    const ASKED_AFTER: std::time::Duration = std::time::Duration::from_secs(5);

    /// The person asked for a tab of theirs with "+", in the project of group `group`: opened on
    /// a thread of its own, since the window's loop calls this and must never wait on the work.
    pub(super) fn person_asked(self: &Arc<Self>, group: &str, asked: Asked) {
        let Some(person) = self.person(group) else {
            return;
        };
        let (desk, group) = (Arc::clone(self), crate::projects::key(person.project.id));
        // What the person asks is done in sight: the window goes to its project first (from the
        // Event Center it may be another's).
        if let Some(browser) = &self.browser {
            browser.go(&group);
        }
        let started = std::thread::Builder::new()
            .name("desk-person-asked".into())
            .spawn(move || {
                if !desk.writes_done(Self::ASKED_AFTER) {
                    log::warn!("desk: the writes before what the person asked are not done yet");
                }
                let call = Call {
                    owner: &person.owner,
                    project: &person.project,
                    agent: None,
                    cwd: &person.cwd,
                    vars: person.vars.as_deref(),
                    peek: false,
                    again: None,
                    person: true,
                    window: None,
                    from: None,
                };
                // The person is known as their project's own, as an agent is once it calls: what
                // they open is of that project wherever it is listed (an ended terminal's event).
                crate::lock(&desk.registry).known(&call);
                let done = match &asked {
                    Asked::Page(url) => {
                        let url = url.as_deref().unwrap_or(HOME);
                        (desk.open(&call, &json!({"kind": "browser", "url": url})))
                            .map(|_| ())
                            .map_err(|failure| failure.message().to_string())
                    }
                    Asked::Terminal(folder) => {
                        // Only a folder of the project (a crumb's), as it really is: no `..`
                        // or link leads out of it.
                        let inside = (folder.as_deref())
                            .and_then(|folder| folder.canonicalize().ok())
                            .filter(|folder| folder.starts_with(&person.project.path));
                        let cwd = inside.as_deref().unwrap_or(&person.cwd);
                        desk.person_terminal(&Call { cwd, ..call }, &group, "")
                    }
                    Asked::Agent(command) => desk.person_terminal(&call, &group, command),
                    Asked::Worker { command, title } => {
                        desk.person_terminal_titled(&call, &group, command, title)
                    }
                    Asked::Board => (desk.board(&call, &json!({"op": "show"})))
                        .map(|_| ())
                        .map_err(|failure| failure.message().to_string()),
                    Asked::Again(row) => desk.again(&call, &group, *row),
                    Asked::Project => desk.reopen_project(&call, &group),
                    Asked::Window => desk.new_window(&call, &group),
                };
                if let Err(why) = done {
                    log::warn!("a {asked:?} the person asked for did not open: {why}");
                }
            });
        if let Err(why) = started {
            log::warn!("a tab the person asked for had no thread: {why}");
        }
    }
}
