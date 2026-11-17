//! Find in what the window shows (Find…, Find Next, Find Previous of the Edit menu), through the
//! one find row, the browser toolbar's, told the step to take (`findText`). Over a web page it
//! lies under the address: the page is someone else's, so nothing of ours is drawn in it. Under
//! a terminal it lies alone across the foot of its pane, the terminal over it. The toolbar says
//! each step of its row back (`Seek`): a page's probe finds and answers what it found (`Found`);
//! a terminal finds in its own text and counts at once. The window keeps each tab's query and
//! count while its row is open: a tab shown again shows its row as it was, and a new document of
//! a page is searched again as it finishes loading.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::{Deck, Held, Part, Shown};
use crate::browser::host::Shared;
use crate::browser::strip::{self, Finding, Found, Key, NAV, Seek, Seeking};
use crate::layout::geometry::Rect;
use crate::surface::{Count, FindStep};

/// How tall the find row is (frame.css `.finds`): the toolbar grows by it over a page, and a
/// terminal's screen gives it the foot of its pane.
const FIND_ROW: f64 = 40.0;

/// What finds in the tab shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Finder {
    /// A terminal: the row under it, its own text searched.
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

/// A tab's find while its row is open: what the row holds, and what was last found of it (none
/// yet for a query a page has not answered).
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

    /// What a terminal's find counted: the match shown (from 1, none: 0) of how many.
    fn counted(&mut self, count: Count) {
        let at = i64::try_from(count.at).unwrap_or(i64::MAX) - 1;
        self.counted = Some((at, u32::try_from(count.total).unwrap_or(u32::MAX)));
    }

    /// What the toolbar is told of the row of tab `tab`: under a terminal (`term`) it shows the
    /// row alone.
    fn told(&self, tab: Key) -> Value {
        let (at, of) = self.counted.unzip();
        let term = matches!(tab, Key::Term(_));
        json!({ "tab": tab.name(), "term": term, "query": self.query, "at": at, "of": of })
    }
}

/// A row's step as a terminal takes it, given what the row held before: a query typed, or a step
/// over one other than the row held, is found anew.
fn step_of(step: Seeking, query: &str, sought: &Sought) -> FindStep {
    match step {
        Seeking::Next if sought.query == query => FindStep::Next,
        Seeking::Previous if sought.query == query => FindStep::Previous,
        Seeking::Type | Seeking::Next | Seeking::Previous => FindStep::Needle(query.to_owned()),
    }
}

/// Where a terminal's find row lies in its `pane`: across its foot.
fn row_under(pane: Rect) -> Rect {
    Rect::new(
        pane.x,
        pane.y + pane.height - FIND_ROW,
        pane.width,
        FIND_ROW,
    )
}

/// Each tab's find while its row is open.
pub(super) type Finds = HashMap<Key, Sought>;

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
        let view = self.finder().and(self.nav.as_ref());
        if let Some(view) = view {
            let _ = view.focus();
            let call = format!("window.findText && findText({})", json!(step.word()));
            let _ = view.evaluate_script(&call);
        }
    }

    /// How tall the browser's toolbar is over the page shown: its bar, and its find row while
    /// that page's is open; nothing where there is no toolbar (a headless window).
    pub(super) fn toolbar(&self) -> f64 {
        let open = (self.web.as_ref())
            .is_some_and(|web| self.finds.contains_key(&Key::Web(web.active_id())));
        match (self.nav.is_some(), open) {
            (false, _) => 0.0,
            (true, false) => NAV,
            (true, true) => NAV + FIND_ROW,
        }
    }

    /// Where the find row of the terminal shown lies while it is open: across the foot of its
    /// pane (`pane`, else the tabs' `whole` part).
    pub(super) fn term_row(&self, pane: Option<Rect>, whole: Rect) -> Option<Rect> {
        let Some(Finder::Panel(id)) = self.finder() else {
            return None;
        };
        self.finds
            .contains_key(&Key::Term(id))
            .then(|| row_under(pane.unwrap_or(whole)))
    }

    /// How much of terminal `id`'s pane its find row takes from its screen.
    pub(super) fn find_under(&self, id: u32) -> f64 {
        let open = self.finder() == Some(Finder::Panel(id));
        match open && self.finds.contains_key(&Key::Term(id)) {
            true => FIND_ROW,
            false => 0.0,
        }
    }

    /// A step of the toolbar's find row in the tab shown, with the run's call `key`: its query
    /// kept for the tab, the row laid out as it opens, or the row shut (its marks gone, the keys
    /// back in the page or the terminal).
    pub(in super::super) fn seek(&mut self, seek: Seek, key: &str) {
        let terms: Vec<u32> = self.terms.iter().map(|open| open.id).collect();
        let web = self.web.as_ref();
        self.finds.retain(|tab, _| match tab {
            Key::Web(id) => web.is_some_and(|web| web.has(*id)),
            Key::Term(id) => terms.contains(id),
        });
        match self.finder() {
            Some(Finder::Page) => self.seek_page(seek, key),
            Some(Finder::Panel(id)) => self.seek_term(id, seek),
            None => return,
        }
        self.tell_find();
    }

    /// A step of the row over the page shown.
    fn seek_page(&mut self, seek: Seek, key: &str) {
        let Some(web) = self.web.as_ref() else {
            return;
        };
        let (id, page) = (web.active_id(), web.active());
        let opening = !self.finds.contains_key(&Key::Web(id));
        match seek {
            Seek::Close => {
                self.finds.remove(&Key::Web(id));
                let _ = page.evaluate_script(&strip::find(key, id, "clear", "", ""));
                self.layout();
                let _ = page.focus();
            }
            Seek::Step {
                step,
                accent,
                query,
            } => {
                let sought = self.finds.entry(Key::Web(id)).or_default();
                sought.asked(&query, &accent);
                let _ = page.evaluate_script(&strip::find(key, id, step.word(), &query, &accent));
                if opening {
                    self.layout();
                }
            }
        }
    }

    /// A step of the row under terminal `id`: found in its text, which lights its matches in the
    /// system's accent, and counted at once; shut, its marks go and it has the keys again.
    fn seek_term(&mut self, id: u32, seek: Seek) {
        let tab = Key::Term(id);
        let opening = !self.finds.contains_key(&tab);
        let Some(view) = self.term(id).and_then(|open| open.face.term()) else {
            return;
        };
        match seek {
            Seek::Close => {
                view.find(&FindStep::Close);
                self.finds.remove(&tab);
                self.layout();
                if let Some(open) = self.term(id) {
                    open.face.focus();
                }
            }
            Seek::Step {
                step,
                accent,
                query,
            } => {
                let was = self.finds.get(&tab).cloned().unwrap_or_default();
                let count = view.find(&step_of(step, &query, &was));
                let sought = self.finds.entry(tab).or_default();
                sought.asked(&query, &accent);
                sought.counted(count);
                if opening {
                    self.layout();
                }
            }
        }
    }

    /// Terminal `id` drew new output while the row under it is open: the matches in it are
    /// counted, the one shown kept, and the row told the count when it changed. Asked as the
    /// terminal says it changed, never on a clock.
    pub(in super::super) fn recount(&mut self, id: u32) {
        let tab = Key::Term(id);
        if self.finder() != Some(Finder::Panel(id)) || !self.finds.contains_key(&tab) {
            return;
        }
        let Some(view) = self.term(id).and_then(|open| open.face.term()) else {
            return;
        };
        let count = view.find(&FindStep::Again);
        let Some(sought) = self.finds.get_mut(&tab) else {
            return;
        };
        let before = sought.counted;
        sought.counted(count);
        if sought.counted != before {
            self.tell_find();
        }
    }

    /// What a page found: its row shows the count, if it is still open and on that query.
    pub(in super::super) fn found(&mut self, found: &Found) {
        let page = self.finds.get_mut(&Key::Web(found.tab));
        if page.is_some_and(|sought| sought.found(found)) {
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
        let Some(sought) = self.finds.get(&Key::Web(id)) else {
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
        let tab = match self.finder() {
            Some(Finder::Page) => self.web.as_ref().map(|web| Key::Web(web.active_id())),
            Some(Finder::Panel(id)) => Some(Key::Term(id)),
            None => None,
        };
        let state = tab.and_then(|tab| self.finds.get(&tab).map(|sought| sought.told(tab)));
        let told = format!("FS({})", crate::page::json(&state.unwrap_or(Value::Null)));
        self.say((Part::Nav, nav), "find", &told);
    }
}

#[cfg(test)]
#[path = "deck_find_tests.rs"]
mod tests;
