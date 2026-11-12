//! What the work thread asks of the window through [`Browser`]: open it, add a panel, list,
//! show, close and size its tabs, and hear what the person does in it.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use crate::view::{ViewError, ViewResult};
use serde_json::Value;

use crate::WindowTab;

use super::super::panel::Panel;
use super::super::strip::{Click, Key};
use super::super::webview::WebviewDriver;
use super::{Browser, Cmd, Dressing, Heard, NAV_PAGE, STRIP, TABS, Window};

impl Browser {
    /// Open the window on `window.url` and wait for its document. One window per process: the
    /// browser shows one page.
    pub fn open(&self, window: Window) -> ViewResult<WebviewDriver> {
        let (reply, opened) = mpsc::sync_channel(1);
        self.proxy
            .send_event(Cmd::Open { window, reply })
            .map_err(|error| {
                ViewError::Unavailable(format!("the browser host is gone: {error}"))
            })?;
        opened
            .recv()
            .map_err(|error| ViewError::Unavailable(format!("the browser host is gone: {error}")))?
            .map_err(|error| {
                ViewError::Unavailable(format!("could not open the browser window: {error}"))
            })?;
        Ok(WebviewDriver::new(
            self.proxy.clone(),
            Arc::clone(&self.shared),
        ))
    }

    /// A driver for the window as it is open now, for a client that did not open it (the app
    /// has one window, and every client of it works in tabs of its own); `None` while it is not.
    pub fn attach(&self) -> Option<WebviewDriver> {
        self.shared
            .open
            .load(Ordering::SeqCst)
            .then(|| WebviewDriver::new(self.proxy.clone(), Arc::clone(&self.shared)))
    }

    /// A tab `id` (unique among the panels: a terminal session's id) titled `title` in the window, showing `html` (a page of the caller's, which paints
    /// run frames in), shown once it opens; it closes when the panel is dropped. It opens the
    /// window when the browser has not.
    pub fn panel(&self, id: u32, title: &str, html: String) -> Panel {
        let proxy = self.proxy.clone();
        let panel = Panel::new(id, Box::new(move |cmd| proxy.send_event(cmd).is_ok()));
        let slot = Arc::clone(&panel.slot);
        let open = Cmd::Panel((id, title.to_string(), html, slot));
        if self.proxy.send_event(open).is_err() {
            panel.slot.alive.store(false, Ordering::SeqCst);
        }
        panel
    }

    /// For checks: how the window is dressed: its materials, the first one's state, whether
    /// the window is opaque, the shape its resizing keeps (width over height, 0 for none),
    /// whether it is coated. `None` while it is closed.
    #[doc(hidden)]
    pub fn dressed(&self) -> Option<Dressing> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.proxy.send_event(Cmd::Dressed(reply)).ok()?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()?
    }

    /// Hear what the person does in the window: a tab or the window closed, by key (`window` for
    /// the whole of it) and in words. It runs on the window's thread: it must not wait.
    pub fn listen(&self, listener: impl Fn(&str, Heard<'_>) + Send + Sync + 'static) {
        *crate::lock(&self.shared.listener) = Some(Arc::new(listener));
    }

    /// The browser tabs open now (`w1`…), as the window last said: no trip to its loop.
    pub fn pages(&self) -> Vec<String> {
        (crate::lock(&self.shared.tabs).iter())
            .map(|tab| Key::Web(tab.id).name())
            .collect()
    }

    /// The window's tabs, in the strip's order; `None` while it is closed.
    pub fn listing(&self) -> Option<Vec<WindowTab>> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.proxy.send_event(Cmd::Listing(reply)).ok()?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()?
    }

    /// Show the tab `id` (`w1`, `t2`), or take it out of the window (`close`).
    pub fn act(&self, id: &str, close: bool) -> Result<(), String> {
        let key = Key::parse(id).ok_or_else(|| format!("there is no tab {id}"))?;
        let click = if close {
            Click::Close(key)
        } else {
            Click::Show(key)
        };
        let (reply, answer) = mpsc::sync_channel(1);
        self.proxy
            .send_event(Cmd::Act(click, reply))
            .map_err(gone)?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(gone)?
    }

    /// For checks: the window's state (`hidden`, `away`, `expanded`); `None` once the loop is gone.
    #[doc(hidden)]
    pub fn presented(&self) -> Option<&'static str> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.proxy.send_event(Cmd::Presented(reply)).ok()?;
        answer.recv_timeout(std::time::Duration::from_secs(5)).ok()
    }

    /// Whether the window is there for the person, in sight or put away (with no tab in it too).
    pub fn shown(&self) -> bool {
        self.presented().is_some_and(|state| state != "hidden")
    }

    /// `question` asked of the person: whether they went on. Waits for their answer, however long
    /// they take; none (the window's loop gone) is no.
    pub fn confirm(&self, question: &super::Confirm) -> bool {
        let (reply, answer) = mpsc::sync_channel(1);
        if self
            .proxy
            .send_event(Cmd::Confirm(question.clone(), reply))
            .is_err()
        {
            return false;
        }
        answer.recv().unwrap_or(false)
    }

    /// The person opened the app: the window comes back, if they put it away, and takes their
    /// keys.
    pub fn present(&self) {
        let _ = self.proxy.send_event(Cmd::Opened);
    }

    /// For checks: what the person's click on the strip says (`show t1`, `close w2`).
    #[doc(hidden)]
    pub fn click(&self, said: &str) {
        let _ = self.proxy.send_event(Cmd::Clicked(said.to_string()));
    }

    /// The Event Center's `events` (a JSON list, newest first), shown under the strip's end.
    pub fn center(&self, events: String) {
        *crate::lock(&self.shared.center) = events;
        let _ = self.proxy.send_event(Cmd::Strip);
    }

    /// Terminal `key` runs `program` in front now (none: none known): a coding agent there is
    /// shown as such, and its integration offered when it is off.
    pub fn runs(&self, key: &str, program: Option<&str>) {
        if self.shared.offers.runs(key, program) {
            let _ = self.proxy.send_event(Cmd::Strip);
        }
    }

    /// Terminal `key` is in `folder` now (its shell after a `cd`, or the program in front's).
    pub fn folder(&self, key: &str, folder: Option<std::path::PathBuf>) {
        let mut folders = crate::lock(&self.shared.folders);
        let changed = match folder {
            Some(folder) => folders.insert(key.to_string(), folder.clone()) != Some(folder),
            None => folders.remove(key).is_some(),
        };
        drop(folders);
        if changed {
            let _ = self.proxy.send_event(Cmd::Strip);
        }
    }

    /// What tab `key`'s program says it is doing (a terminal's title), shown under its name;
    /// `None` forgets it, for a tab that is gone.
    pub fn detail(&self, key: &str, detail: Option<&str>) {
        let mut details = crate::lock(&self.shared.details);
        let changed = match detail {
            Some(detail) => {
                details
                    .insert(key.to_string(), detail.to_string())
                    .as_deref()
                    != Some(detail)
            }
            None => details.remove(key).is_some(),
        };
        drop(details);
        if changed {
            let _ = self.proxy.send_event(Cmd::Strip);
        }
    }

    /// Where the person's answers to offers are kept: the window offers nothing before.
    pub fn answers(&self, answers: Arc<dyn super::Answers>) {
        self.shared.offers.keep(answers);
    }

    /// Project `project`'s past (its key's): the history menu lists its tabs, the address
    /// field suggests its pages.
    pub fn ledger(&self, project: &str, ledger: super::Ledger) {
        crate::lock(&self.shared.ledgers).insert(project.to_string(), ledger);
        let _ = self.proxy.send_event(Cmd::Strip);
    }

    /// News of project `project`'s tree of files (its key): held for the windows that show
    /// it, and drawn in their sides.
    pub fn tree(&self, project: &str, tree: crate::browser::Tree) {
        let news = super::super::tree::news(project, &tree);
        crate::lock(&self.shared.trees).take(project, tree);
        let _ = self.proxy.send_event(Cmd::Tree(project.to_string(), news));
    }

    /// The window's projects' tabs as the layout keeps them: their order, their splits, the one
    /// each shows. The window draws them so.
    pub fn arranged(&self, windows: Vec<i64>, arranged: Vec<super::Arranged>) {
        let _ = self.proxy.send_event(Cmd::Arranged(windows, arranged));
    }

    /// The app opened window `window` (its row in the layout): the window in use from now on,
    /// made with the first tab the app opens next.
    pub fn new_window(&self, window: i64) {
        let _ = self.proxy.send_event(Cmd::NewWindow(window));
    }

    /// The window the person works in, by its row in the layout; none before the app named it.
    pub fn in_use(&self) -> Option<i64> {
        let window = self.shared.in_use.load(Ordering::SeqCst);
        (window != 0).then_some(window)
    }

    /// The person shows tab `id`: the window goes to its project, then to it.
    pub fn reveal(&self, id: &str) -> Result<(), String> {
        let project = crate::lock(&self.shared.groups).get(id).cloned();
        if let Some(project) = project.filter(|project| !project.is_empty()) {
            self.go(&project);
        }
        self.act(id, false)
    }

    /// The window goes to project `project` (its key): its last tab shown, else the app is asked
    /// for what it had open.
    pub fn go(&self, project: &str) {
        let _ = self.proxy.send_event(Cmd::Go(project.to_string()));
    }

    /// The file at `path` opened by the app the person has for its kind: what no tab shows.
    pub fn outside(&self, path: &std::path::Path) {
        let _ = self.proxy.send_event(Cmd::Outside(path.to_path_buf()));
    }

    /// The next tab the app opens is of project `project` (its key): out of sight, and that
    /// project marked, when the window shows another. `aside`, it opens beside a tab in sight
    /// (the layout says which) and does not take the window. The app opens one tab at a time.
    pub fn opening(&self, project: &str, aside: bool) {
        crate::lock(&self.shared.scope).opening(project, aside);
    }

    /// Terminal or board `key`, about to open, opens beside a tab in sight: it does not take
    /// the window.
    pub fn aside(&self, key: &str) {
        crate::lock(&self.shared.scope).aside(key);
    }

    /// The projects the window knows (`group` keys tabs by theirs), most recent first: what
    /// it calls them, and their folders.
    pub fn places(&self, places: Vec<super::Place>) {
        *crate::lock(&self.shared.places) = places;
        let _ = self.proxy.send_event(Cmd::Strip);
    }

    /// Tab `key` (`w1`, `t2`) is of `group`, a project: the strip draws each one's tabs
    /// together once there are several.
    pub fn group(&self, key: &str, group: &str) {
        crate::lock(&self.shared.groups).insert(key.to_string(), group.to_string());
        let _ = self.proxy.send_event(Cmd::Strip);
    }

    /// What `script` evaluates to in `panel`'s page, as JSON.
    pub fn peek(&self, panel: &Panel, script: &str) -> Result<Value, String> {
        self.peek_in(panel.id(), script)
    }

    /// For checks: what `script` evaluates to in the strip's page.
    #[doc(hidden)]
    pub fn peek_strip(&self, script: &str) -> Result<Value, String> {
        self.peek_in(STRIP, script)
    }

    /// For checks: what `script` evaluates to in the page that lists the tabs (the side, or the
    /// strip where there is none).
    #[doc(hidden)]
    pub fn peek_tabs(&self, script: &str) -> Result<Value, String> {
        self.peek_in(TABS, script)
    }

    /// For checks: what `script` evaluates to in the browser's toolbar's page.
    #[doc(hidden)]
    pub fn peek_nav(&self, script: &str) -> Result<Value, String> {
        self.peek_in(NAV_PAGE, script)
    }

    fn peek_in(&self, panel: u32, script: &str) -> Result<Value, String> {
        let call = self.shared.calls.open();
        let (id, receiver) = (call.id, call.receiver);
        let script = script.to_string();
        let panel = Some(panel);
        let _ = (self.proxy).send_event(Cmd::Eval {
            id,
            script,
            failed: "",
            panel,
            tab: None,
        });
        let reply = receiver.recv_timeout(std::time::Duration::from_secs(5));
        self.shared.calls.forget(id);
        reply.unwrap_or_else(|error| Err(format!("no answer: {error}")))
    }
}

/// Why the window could not be asked: it is not open, and how that showed.
fn gone(error: impl std::fmt::Display) -> String {
    format!("the window is not open: {error}")
}
