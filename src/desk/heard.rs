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
                    return self.unkeep(key);
                }
                let owner = self.owner_of(key).filter(|_| !person);
                if let Some(id) = key.strip_prefix('w').and_then(|n| n.parse::<u32>().ok()) {
                    crate::lock(&self.registry).release(key);
                    crate::lock(&self.seen).remove(&id);
                }
                self.gone(owner, key);
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
            Heard::Showing => self.showing(key),
            Heard::Split(halves) => self.split_told(key, halves),
            Heard::SplitResized(portion) => {
                let key = key.to_string();
                let desk = Arc::clone(self);
                self.write_later(move || {
                    if let Err(why) = desk.split_resized(&key, portion) {
                        log::warn!("the split size was not kept: {why}");
                    }
                });
            }
            Heard::Moved { before, window } => self.moved(key, before, window),
            Heard::Detached => self.detached(key),
            Heard::WindowClosed(window) => self.window_closed(window),
            Heard::Quit => self.quit_asked(),
            Heard::Reopened => self.reopened(),
            Heard::Asked(asked) => self.person_asked(key, asked),
            Heard::Center(did) => self.person_on_center(did),
        }
    }

    /// How many writes wait for the writer thread at most.
    const WRITES: usize = 256;

    /// `write` done on the desk's writer thread, after what was handed it before: the window's
    /// loop goes on at once. With no thread to be had, it is done here.
    fn write_later(&self, write: impl FnOnce() + Send + 'static) {
        let writes = self.writes.get_or_init(|| {
            let (send, take) =
                std::sync::mpsc::sync_channel::<Box<dyn FnOnce() + Send>>(Self::WRITES);
            let started = std::thread::Builder::new()
                .name("desk-writes".into())
                .spawn(move || {
                    for write in take {
                        // A write that panics is a bug of its own: the ones after it still go.
                        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(write)).is_err() {
                            log::warn!("desk: a write panicked: the writer goes on");
                        }
                    }
                });
            if let Err(why) = started {
                log::warn!("desk: no writer thread ({why}): writes are done in place");
            }
            send
        });
        // A queue full (the disk far behind) or gone: done here, rather than waiting on it.
        match writes.try_send(Box::new(write)) {
            Ok(()) => {}
            Err(
                std::sync::mpsc::TrySendError::Full(write)
                | std::sync::mpsc::TrySendError::Disconnected(write),
            ) => write(),
        }
    }

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
