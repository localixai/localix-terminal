//! Find in what the window shows (Find…, Find Next, Find Previous of the Edit menu). A terminal
//! keeps its own find row, query and count in its page, and is told only the step to take
//! (`findText`). A web page is someone else's, so nothing of ours is drawn in it: its row is the
//! browser toolbar's, told the step the same way. The toolbar says each step of its row back
//! (`Seek`), the page's probe finds and answers what it found (`Found`), and the window keeps
//! each page tab's query and count while its row is open: a tab shown again shows its row as it
//! was, and a new document of it is searched again as it finishes loading.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::{Deck, Held, Part, Shown};
use crate::browser::host::Shared;
use crate::browser::strip::{self, Finding, Found, Key, NAV, Seek};

/// How tall the find row is (frame.css `.finds`): the toolbar grows by it over a page.
const FIND_ROW: f64 = 40.0;

/// What finds in the tab shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Finder {
    /// A terminal's page: it keeps its own row.
    Panel(u32),
    /// The page shown, through the browser's toolbar over it.
    Page,
}

/// Which finds for what is `shown`: a terminal live in its tab (`panel`: how its view is held,
/// and its kind), or a page with the browser's toolbar over it (`toolbar`). Not a board, not
/// Settings, not a terminal closed or ended, not a headless window's page.
fn finder(shown: Shown, panel: Option<(Held, &str)>, toolbar: bool) -> Option<Finder> {
    match shown {
        Shown::Term(id) => matches!(panel, Some((Held::Live, "term"))).then_some(Finder::Panel(id)),
        Shown::Web => toolbar.then_some(Finder::Page),
        Shown::Nothing => None,
    }
}

/// A page tab's find while its row is open: what the row holds, and what the page last found of
/// it (none yet for a query just typed).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Sought {
    query: String,
    accent: String,
    counted: Option<(i64, u32)>,
}

impl Sought {
    /// The row's query and accent, as it says them: a new query is not counted yet.
    fn asked(&mut self, query: &str, accent: &str) {
        if self.query != query {
            query.clone_into(&mut self.query);
            self.counted = None;
        }
        accent.clone_into(&mut self.accent);
    }

    /// What the page found, kept when it is of the query the row holds now (not one typed over).
    fn found(&mut self, found: &Found) -> bool {
        let now = found.query == self.query;
        if now {
            self.counted = Some((found.at, found.of));
        }
        now
    }

    /// What the toolbar is told of the row of tab `tab`.
    fn told(&self, tab: u32) -> Value {
        let (at, of) = self.counted.unzip();
        json!({ "tab": tab, "query": self.query, "at": at, "of": of })
    }
}

/// Each page tab's find while its row is open, by tab id.
pub(super) type Finds = HashMap<u32, Sought>;

impl Deck {
    /// What finds in the tab shown, if anything does.
    fn finder(&self) -> Option<Finder> {
        let panel = match self.shown {
            Shown::Term(id) => self.term(id).map(|open| (open.held, Key::Term(id).kind())),
            Shown::Web | Shown::Nothing => None,
        };
        finder(self.shown, panel, self.page_shown() && self.nav.is_some())
    }

    /// Whether what is shown can find: what the menu's Find items are valid for.
    pub(in super::super) fn finds(&self) -> bool {
        self.finder().is_some()
    }

    /// One step of Find in the tab shown, given the keys first (its find row's field takes
    /// them): a terminal's page, or the toolbar over a page. Nothing while a menu or a dialog
    /// has the window.
    pub(in super::super) fn find_text(&self, step: Finding) {
        if self.covered() {
            return;
        }
        let view = match self.finder() {
            Some(Finder::Panel(id)) => self.term(id).map(|open| &open.webview),
            Some(Finder::Page) => self.nav.as_ref(),
            None => None,
        };
        if let Some(view) = view {
            let _ = view.focus();
            let call = format!("window.findText && findText({})", json!(step.word()));
            let _ = view.evaluate_script(&call);
        }
    }

    /// How tall the browser's toolbar is over the page shown: its bar, and its find row while
    /// that page's is open; nothing where there is no toolbar (a headless window).
    pub(super) fn toolbar(&self) -> f64 {
        let open = (self.web.as_ref()).is_some_and(|web| self.finds.contains_key(&web.active_id()));
        match (self.nav.is_some(), open) {
            (false, _) => 0.0,
            (true, false) => NAV,
            (true, true) => NAV + FIND_ROW,
        }
    }

    /// A step of the toolbar's find row in the page shown, with the run's call `key`: its query
    /// kept for the tab, the row laid out as it opens, or the row shut (its marks gone, the keys
    /// back in the page).
    pub(in super::super) fn seek(&mut self, seek: Seek, key: &str) {
        let Some(web) = self
            .web
            .as_ref()
            .filter(|_| self.finder() == Some(Finder::Page))
        else {
            return;
        };
        let (id, page) = (web.active_id(), web.active());
        self.finds.retain(|tab, _| web.has(*tab));
        let opening = !self.finds.contains_key(&id);
        match seek {
            Seek::Close => {
                self.finds.remove(&id);
                let _ = page.evaluate_script(&strip::find(key, id, "clear", "", ""));
                self.layout();
                let _ = page.focus();
            }
            Seek::Step {
                step,
                accent,
                query,
            } => {
                self.finds.entry(id).or_default().asked(&query, &accent);
                let _ = page.evaluate_script(&strip::find(key, id, step.word(), &query, &accent));
                if opening {
                    self.layout();
                }
            }
        }
        self.tell_find();
    }

    /// What a page found: its row shows the count, if it is still open and on that query.
    pub(in super::super) fn found(&mut self, found: &Found) {
        if (self.finds.get_mut(&found.tab)).is_some_and(|sought| sought.found(found)) {
            self.tell_find();
        }
    }

    /// The toolbar told the page shown's find (its row, or none), and the page told it again
    /// once another tab is shown or a new document of it loaded (`again`: nothing moves).
    pub(super) fn refind(&self, shared: &Shared) {
        self.tell_find();
        let Some(web) = self
            .web
            .as_ref()
            .filter(|_| self.finder() == Some(Finder::Page))
        else {
            return;
        };
        let id = web.active_id();
        let Some(sought) = self.finds.get(&id) else {
            return;
        };
        let loaded = (crate::lock(&shared.loads).get(&id)).map_or(0, |loads| loads.finished);
        let again = strip::find(&shared.key, id, "again", &sought.query, &sought.accent);
        self.tell((None, web.active()), "find", &again, (id, loaded));
    }

    fn tell_find(&self) {
        let Some(nav) = &self.nav else {
            return;
        };
        let state = (self.web.as_ref())
            .filter(|_| self.page_shown())
            .and_then(|web| {
                let id = web.active_id();
                self.finds.get(&id).map(|sought| sought.told(id))
            });
        let told = format!("FS({})", crate::page::json(&state.unwrap_or(Value::Null)));
        self.say((Part::Nav, nav), "find", &told);
    }
}

#[cfg(test)]
#[path = "deck_find_tests.rs"]
mod tests;
