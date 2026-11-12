//! A kind of tab that draws itself: a page of its own in a panel, painted whole by frames, acted
//! on by an agent's `window` calls and by what the person does in its page. A kind says
//! everything the window and the desk need of it in one description ([`About`]) and answers
//! through one port ([`Kind`]); neither names a kind, and a kind names neither. The kinds there
//! are are listed once, where the app is put together (`list_kinds` in `cli.rs`).

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

/// What a kind says of itself: read by the tool's schema and guide, the strip, the "+" menu,
/// the shortcuts and the ledger.
#[derive(Debug, PartialEq, Eq)]
pub struct About {
    /// Its name: the `kind` of a call, what the ledger keeps, what the strip draws it as.
    pub id: &'static str,
    /// The letter of its tabs' keys: `m` for `m1`.
    pub letter: char,
    /// What its tab and its command are called.
    pub title: &'static str,
    /// What its tab holds, in a few words: the line under its name in the side.
    pub holds: &'static str,
    /// Its picture: the data of one stroked path on the strip's 16-point grid.
    pub glyph: &'static str,
    /// The fields the tool takes on its tab, each with its JSON Schema.
    pub fields: &'static [(&'static str, &'static str)],
    /// Its paragraph of the tool's description.
    pub guide: &'static str,
    /// How it is acted on, said after a field it does not take.
    pub instead: &'static str,
    /// What its owner hears when the person closes its tab.
    pub closed: &'static str,
    /// The keys that open it until the person sets their own (`cmd+shift+e`), or none.
    pub keys: &'static str,
}

impl About {
    /// The id of the command that opens it, as the shortcuts name it.
    pub fn command(&self) -> &'static str {
        self.id
    }

    /// What its key is called in the strip's page.
    pub fn keyed(&self) -> String {
        format!("key new {}", self.id)
    }
}

/// A tab of a kind, as the kind may use it.
pub trait Tab: Send + Sync {
    /// Its key: `m1`.
    fn key(&self) -> &str;
    /// Run `frame`, a script, in its page. Nothing is run where no window shows.
    fn paint(&self, frame: &str);
    /// Open `url` in a page of the person's, beside it.
    fn browse(&self, url: &str);
    /// `question` put to the person in the window's dialog, the one every question of the app is
    /// asked in: what they gave for its fields when they went on, none when they did not. It
    /// waits for them, however long: never called on a thread that must go on.
    fn ask(&self, question: &Question) -> Option<Given>;
}

/// What the person gave for a question's fields, by each field's name.
pub type Given = BTreeMap<String, String>;

/// A question a kind's tab puts to the person: a title that asks, a sentence that says what
/// happens, the points of it worth reading first, and what it needs typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Question {
    pub title: String,
    pub detail: String,
    /// What the points are, for assistive tools: `What It Does`.
    pub heading: String,
    pub points: Vec<String>,
    pub fields: Vec<Field>,
    /// The button that does it.
    pub yes: String,
    /// It cannot be undone.
    pub danger: bool,
    /// The person asked for it just now, so it takes the keys; one an agent raised does not.
    pub asked: bool,
}

/// One thing a question needs typed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Field {
    pub name: String,
    /// What is read over it.
    pub label: String,
    /// What stands in it while it is empty.
    pub hint: String,
    /// Shown as dots.
    pub secret: bool,
    /// Going on waits for it.
    pub required: bool,
}

/// What a call on a kind's tab answers: what was done, and its tab as text after.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Said {
    pub report: String,
    pub state: Option<String>,
}

/// A kind of tab. One of it serves every tab of its kind; what a tab shows that another does not
/// is its page's own.
pub trait Kind: Send + Sync {
    fn about(&self) -> &'static About;

    /// Its page, titled `title`: one document under the panels' policy, with no outside resource.
    fn page(&self, title: &str) -> String;

    /// Its page loaded, at first or again: it is painted whole.
    fn loaded(&self, tab: &Arc<dyn Tab>);

    /// A call of an agent on its tab: `args` as the tool took them.
    fn call(&self, args: &Value, tab: &Arc<dyn Tab>) -> Result<Said, String>;

    /// What the person did in its page, as the page posted it.
    fn posted(&self, body: &str, tab: &Arc<dyn Tab>);

    /// Its tab as an agent reads it.
    fn text(&self) -> String;

    /// It in a few words, for a list of tabs.
    fn summary(&self) -> String;
}

/// The kinds of this process, in the order they are listed everywhere.
pub fn all() -> Vec<Arc<dyn Kind>> {
    registry::all()
}

/// The kind named `id`.
pub fn named(id: &str) -> Option<Arc<dyn Kind>> {
    all().into_iter().find(|kind| kind.about().id == id)
}

/// The kind whose tabs' keys begin with `letter`, and its place among the kinds.
pub fn lettered(letter: char) -> Option<(usize, Arc<dyn Kind>)> {
    (all().into_iter().enumerate()).find(|(_, kind)| kind.about().letter == letter)
}

/// What each kind says of itself.
pub fn abouts() -> Vec<&'static About> {
    all().iter().map(|kind| kind.about()).collect()
}

/// The letters of the keys of the tabs the window has without any kind listed: a page, a
/// terminal, the settings, a board and a file.
const TAKEN: &str = "wtsbf";

/// These are the kinds, from now on; in the running app the first list given stays. A kind whose
/// keys could be read as another tab's is not listed: its tab could never be shown.
pub fn install(kinds: Vec<Arc<dyn Kind>>) {
    registry::install(distinct(kinds));
}

/// `kinds` without those whose keys' letter is no lowercase letter, is a tab's the window always
/// had, or is that of a kind listed before it.
fn distinct(kinds: Vec<Arc<dyn Kind>>) -> Vec<Arc<dyn Kind>> {
    let mut letters = String::from(TAKEN);
    (kinds.into_iter())
        .filter(|kind| {
            let about = kind.about();
            let own = about.letter.is_ascii_lowercase() && !letters.contains(about.letter);
            if own {
                letters.push(about.letter);
            } else {
                log::error!(
                    "kind {}: its keys' letter {} is taken",
                    about.id,
                    about.letter
                );
            }
            own
        })
        .collect()
}

/// The kinds as the running app holds them (listed once), and under test (each thread its own).
#[cfg(not(test))]
mod registry;
#[cfg(test)]
#[path = "kind/registry_testing.rs"]
mod registry;

#[cfg(test)]
pub mod testing;
#[cfg(test)]
#[path = "kind_tests.rs"]
mod tests;
