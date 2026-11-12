//! The browser window and the page it shows, as a `View`: the window opens on the first address
//! it is sent to, and later addresses load in it. The browser's own toolbar (Back, Reload) is part
//! of the view, acted on by its refs like any control.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use crate::view::{Action, Observation, View, ViewError, ViewResult};

use super::{Browser, Driver, Viewport, WebviewDriver, Window, toolbar};

/// The window as a view. Its lock is held only to look at the window or change what is held of
/// it, never across a load or a reply of the window's loop: a page that
/// loads still answers other threads. Calls in one tab are one at a time by the caller's choice
/// (the app's lock per tab).
pub struct Page {
    browser: Browser,
    /// Hidden unless a call asks to show it: the default the app chose for this run.
    headless: bool,
    held: Mutex<Held>,
    /// Rung when an opening ends, for the calls that wait to open the window.
    opened: Condvar,
}

/// The window as a page holds it.
struct Held {
    driver: Option<WebviewDriver>,
    /// A call is opening the window: the others wait for it rather than open a second one.
    opening: bool,
}

/// What a call that wants the window found, once no other call was opening it.
enum Claim {
    /// The window, open.
    Open(WebviewDriver),
    /// None open: this call opens it.
    Make,
}

impl Page {
    pub fn new(browser: Browser, headless: bool) -> Self {
        Self {
            browser,
            headless,
            held: Mutex::new(Held {
                driver: None,
                opening: false,
            }),
            opened: Condvar::new(),
        }
    }

    /// Load `url`, opening the window on it the first time (and again after the person closed
    /// it).
    pub fn open(&self, url: &str) -> ViewResult<()> {
        match self.claim() {
            Claim::Open(open) => open.navigate(url),
            Claim::Make => {
                let (driver, tab) = self.make(url)?;
                // A slow page is used as it is: it says it is busy until it settles.
                driver.on(tab).wait_new();
                Ok(())
            }
        }
    }

    /// Open `url` in a tab of its own and say the tab's id, its document not waited for (a driver
    /// pinned to the tab waits: `wait_new`): a new tab of the window open, else the window opened
    /// on it.
    pub fn start(&self, url: &str) -> ViewResult<u32> {
        match self.claim() {
            Claim::Open(open) => open.add_tab(url),
            Claim::Make => self.make(url).map(|made| made.1),
        }
    }

    /// Once no call is opening the window: the window if it is open, else this call's claim to
    /// open it. The look and the claim are one step under the lock, so two calls never open two
    /// windows; the opening itself is done with the lock let go, while the calls that want the
    /// window wait on `opened`.
    fn claim(&self) -> Claim {
        let held = self.held();
        let mut held = (self.opened)
            .wait_while(held, |held| held.opening)
            .unwrap_or_else(PoisonError::into_inner);
        match held.driver.clone().filter(WebviewDriver::is_open) {
            Some(open) => Claim::Open(open),
            None => {
                held.opening = true;
                Claim::Make
            }
        }
    }

    /// The window made for `url` by the call that claimed it: a tab of its own in the window open
    /// for someone else (another client of the app, whose tabs stay as they are), else the window
    /// opened on it. Its driver is held at once and the claim ends, however this does; the tab's
    /// document is not waited for. The driver and the tab's id.
    fn make(&self, url: &str) -> ViewResult<(WebviewDriver, u32)> {
        let _claim = Claimed(self);
        let attach = |attached: WebviewDriver| {
            let tab = (attached.add_tab(url))
                .map_err(|error| ViewError::Unavailable(error.to_string()))?;
            self.held().driver = Some(attached.clone());
            Ok((attached, tab))
        };
        if let Some(attached) = self.browser.attach() {
            return attach(attached);
        }
        let opened = match self.browser.open(Window {
            url: url.to_string(),
            title: url.to_string(),
            headless: self.headless,
            viewport: Viewport::DESKTOP,
        }) {
            Ok(opened) => opened,
            // Another page opened it between the look and this: a tab of its own in it.
            Err(refused) => return self.browser.attach().map_or(Err(refused), attach),
        };
        let (_, tab) = opened.tabs();
        self.held().driver = Some(opened.clone());
        Ok((opened, tab))
    }

    pub fn is_open(&self) -> bool {
        (self.held().driver.as_ref()).is_some_and(WebviewDriver::is_open)
    }

    /// The open window's driver, for what only a browser has (scripts, keys, the whole text).
    pub fn driver(&self) -> ViewResult<WebviewDriver> {
        let driver = self.held().driver.clone();
        driver
            .filter(WebviewDriver::is_open)
            .ok_or_else(|| ViewError::Unavailable("the browser window is not open".into()))
    }

    /// The same window acting in tab `id`, whatever is shown: a page of its own to work in
    /// beside the others.
    #[must_use]
    pub fn pinned(&self, id: u32) -> Page {
        let driver = self.held().driver.as_ref().map(|d| d.on(id));
        Page {
            browser: self.browser.clone(),
            headless: self.headless,
            held: Mutex::new(Held {
                driver,
                opening: false,
            }),
            opened: Condvar::new(),
        }
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        crate::lock(&self.held)
    }

    /// A toolbar control: back or reload.
    fn toolbar(&self, action: &Action) -> ViewResult<()> {
        let driver = self.driver()?;
        match action {
            Action::Click { reference } if reference == toolbar::BACK => {
                driver.evaluate("history.back()").map(|_| ())
            }
            Action::Click { reference } if reference == toolbar::RELOAD => driver.reload(),
            Action::Click { .. }
            | Action::Select { .. }
            | Action::Type { .. }
            | Action::PressEnter { .. }
            | Action::ScrollDown
            | Action::ScrollUp
            | Action::Press { .. }
            | Action::Go { .. } => Err(ViewError::Refused("a toolbar control is clicked".into())),
        }
    }

    /// Give the page a size of its own (`iPhone 15 Pro`, `mobile`, `393x852`; `window` for the
    /// whole window); what it is now, in words.
    pub fn resize(&self, size: &str) -> ViewResult<String> {
        self.driver()?.frame(size)
    }
}

/// A call's claim to open the window, ended however its opening ends: the calls waiting for it
/// are woken.
struct Claimed<'a>(&'a Page);

impl Drop for Claimed<'_> {
    fn drop(&mut self) {
        self.0.held().opening = false;
        self.0.opened.notify_all();
    }
}

impl View for Page {
    fn observe(&self) -> ViewResult<Observation> {
        let driver = self.driver()?;
        let mut view = Observation::from(driver.observe()?);
        let controls = toolbar::controls();
        let bar = toolbar::outline(&controls);
        view.outline = format!("{bar}\n{}", view.outline);
        view.elements.splice(0..0, controls);
        Ok(view)
    }

    fn epoch(&self) -> ViewResult<String> {
        self.driver()?.epoch()
    }

    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()> {
        let reference = match action {
            Action::Click { reference }
            | Action::Type { reference, .. }
            | Action::Select { reference, .. } => Some(reference.as_str()),
            Action::PressEnter { .. } | Action::ScrollDown | Action::ScrollUp => None,
            // The same paths as a call's `keys` and `url`.
            Action::Press { key } => {
                return self.driver()?.press(std::slice::from_ref(key), 0);
            }
            Action::Go { url } => return self.open(url),
        };
        match reference {
            Some(reference) if toolbar::owns(reference) => self.toolbar(action),
            _ => self.driver()?.act(epoch, action),
        }
    }
}
