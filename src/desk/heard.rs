//! Window events adapted into desk actions, with disk writes kept off the UI thread.

use std::sync::Arc;

use serde_json::json;

use crate::browser::{Asked, HOME, Heard, Side, Went};

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
                // The tab of a kind that draws itself went with its page.
                if self.forget_made(key) {
                    crate::lock(&self.registry).release(key);
                }
                if key.starts_with('f') {
                    // Its page went with its tab: its watch and worker go off the window's loop.
                    crate::lock(&self.registry).release(key);
                    let (desk, file) = (Arc::clone(self), key.to_string());
                    self.write_later(move || {
                        desk.forget_file(&file);
                    });
                }
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
            Heard::SplitNew {
                beside,
                side,
                asked,
            } => self.person_asked_beside(key, asked, Some((beside.to_string(), side))),
            Heard::Split { beside, side } => self.split_later(key, beside, side),
            Heard::Whole => self.whole_later(key),
            Heard::Equalized => self.equalized_later(key),
            Heard::Resized { path, at, share } => self.resized_later(key, path, at, share),
            Heard::Focused => self.focused_later(key),
            Heard::Zoomed(on) => self.zoomed_later(key, on),
            // The panes are found by their rows as the window tells it; the layout is written
            // after what the window told before.
            Heard::Moved { before, window } => {
                if let Some(rows) = self.rows_moved(key, before) {
                    let (desk, key) = (Arc::clone(self), key.to_string());
                    self.write_later(move || desk.move_kept(&key, rows, window));
                }
            }
            Heard::Placed(before) => {
                if let Some(rows) = self.rows_placed(key, before) {
                    let (desk, key) = (Arc::clone(self), key.to_string());
                    self.write_later(move || desk.place_kept(&key, rows));
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
            Heard::Update => self.update_asked(),
            Heard::Reopened => self.reopened(),
            Heard::Asked(asked) => self.person_asked(key, asked),
            Heard::Center(did) => self.person_on_center(did),
            Heard::Tree(did) => self.person_on_tree(key, did),
            Heard::Unattended { code, took } => self.unattended(key, Some((code, took))),
            Heard::Rang => self.unattended(key, None),
        }
    }

    /// A command the person typed in terminal `key` ended (`ended`: its code and how long it
    /// took), or its program rang the bell (none), while they were not looking: kept in its
    /// session until they type there, and news for the Event Center.
    fn unattended(&self, key: &str, ended: Option<(Option<i32>, std::time::Duration)>) {
        let Some(id) = key.strip_prefix('t').and_then(|n| n.parse().ok()) else {
            return;
        };
        let Ok(session) = self.session(id) else {
            return;
        };
        match ended {
            Some((code, took)) => session.unseen(crate::terminal::Ended { code, took }),
            None => session.rang_unseen(),
        }
        let feed = self.feed.clone();
        self.write_later(move || {
            feed.record(
                &crate::feed::Entity::Terminal { id },
                crate::feed::Op::Upserted,
            );
        });
    }

    /// How long what the person asks for waits for the writes the window told before it: a tab
    /// they closed a moment ago is closed in the ledger before they open it again.
    const ASKED_AFTER: std::time::Duration = std::time::Duration::from_secs(5);

    /// The person asked for a tab of theirs with "+", in the project of group `group`.
    pub(super) fn person_asked(self: &Arc<Self>, group: &str, asked: Asked) {
        self.person_asked_beside(group, asked, None);
    }

    /// What the person asked for, in the project of group `group` (`beside`: a new pane on that
    /// side of that pane, in its tab): opened on a thread of its own, since the window's loop
    /// calls this and must never wait on the work.
    pub(super) fn person_asked_beside(
        self: &Arc<Self>,
        group: &str,
        asked: Asked,
        beside: Option<(String, Side)>,
    ) {
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
                    runs: None,
                };
                // The person is known as their project's own, as an agent is once it calls: what
                // they open is of that project wherever it is listed (an ended terminal's event).
                crate::lock(&desk.registry).known(&call);
                let done = match &beside {
                    Some((key, side)) => {
                        (desk.person_beside(&call, &group, (key, *side), &asked)).map(drop)
                    }
                    None => desk.person_does(&call, &group, &asked),
                };
                if let Err(why) = done {
                    log::warn!("a {asked:?} the person asked for did not open: {why}");
                }
            });
        if let Err(why) = started {
            log::warn!("a tab the person asked for had no thread: {why}");
        }
    }

    /// What the person asked for done, as `call` in the project of group `group`.
    pub(super) fn person_does(
        &self,
        call: &Call<'_>,
        group: &str,
        asked: &Asked,
    ) -> Result<(), String> {
        let call = *call;
        match asked {
            Asked::Page(url) => {
                let url = url.as_deref().unwrap_or(HOME);
                (self.open(&call, &json!({"kind": "browser", "url": url})))
                    .map(|_| ())
                    .map_err(|failure| failure.message().to_string())
            }
            Asked::Terminal(folder) => {
                // Only a folder of the project (a crumb's), as it really is: no `..` or link
                // leads out of it.
                let inside = (folder.as_deref())
                    .and_then(|folder| folder.canonicalize().ok())
                    .filter(|folder| folder.starts_with(&call.project.path));
                let cwd = inside.as_deref().unwrap_or(call.cwd);
                self.person_terminal(&Call { cwd, ..call }, group, "")
            }
            Asked::Agent(command) => self.person_terminal(&call, group, command),
            Asked::Worker {
                command,
                title,
                who,
            } => (self.worker_terminal(&call, group, command, title, who)).map(drop),
            Asked::Session {
                command,
                title,
                folder,
                waits,
            } => {
                // Its own folder, where its agent keeps it: never another, whose last session
                // would be continued in its place.
                let inside = (folder.canonicalize().ok())
                    .filter(|folder| folder.starts_with(&call.project.path));
                match inside {
                    Some(cwd) => {
                        let call = Call { cwd: &cwd, ..call };
                        (self.person_terminal_titled(&call, group, command, title))
                            .inspect(|()| self.continued_here(waits))
                    }
                    None => Err(format!("{} is no folder of it", folder.display())),
                }
            }
            Asked::File(path) => self.person_file(&call, path),
            Asked::Text(path) => self.person_text(&call, path),
            Asked::Board => (self.board(&call, &json!({"op": "show"})))
                .map(|_| ())
                .map_err(|failure| failure.message().to_string()),
            Asked::Kind(kind) => match self.kind_named(kind) {
                Some(about) => (self.made(&call, about, &json!({})))
                    .map(|_| ())
                    .map_err(|failure| failure.message().to_string()),
                None => Err(format!("there is no kind of tab `{kind}`")),
            },
            Asked::Again(row) => self.again(&call, group, *row),
            Asked::Project => self.reopen_project(&call, group),
            Asked::Window => self.new_window(&call, group),
        }
    }
}
