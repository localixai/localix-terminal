//! The Store's tab: a kind of tab that draws itself (`crate::kind`). One Store serves every
//! tab of it: the index it searches, the note of what it installed, and what it is doing now.
//! The person searches, reads and installs in its page (kind/person.rs); an agent searches and
//! reads, and what it asks to install is put to the person first (kind/agent.rs). Whatever
//! takes time, a read of the sources or an install, is done off the tab's thread (kind/work.rs).

mod agent;
mod asking;
mod person;
mod sourced;
#[cfg(test)]
mod testing;
mod work;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use serde_json::{Value, json};

use super::installed::Installed;
use super::page;
use super::sources::Sources;
use super::{Env, Fetch, Index, Listing, Source, What, target::Target};
use crate::kind::{About, Kind, Said, Tab};

static ABOUT: About = About {
    id: "store",
    letter: 'p',
    title: "Store",
    holds: "Plugins, skills and MCP servers",
    glyph: "M3.5 5.5h9l-.7 8H4.2zM6 5.5v-1a2 2 0 0 1 4 0v1",
    fields: &[(
        "store",
        r#"{"type": "object", "description": "store: {\"search\": \"words\", \"what\": \"plugin|skill|server\", \"limit\": 20} lists what matches; {\"show\": \"<id>\"} reads one listing and which agents have it; {\"install\": \"<id>\", \"agent\": \"claude|codex|gemini|copilot|opencode\"} asks the person to install it.", "additionalProperties": true}"#,
    )],
    guide: "store: the catalogue of plugins, skills and MCP servers for coding agents. Search and \
            read it with `store`. An install is the person's to allow: `install` shows them what \
            it would write or run, in the Store's tab, and the tab read again says what they \
            answered. Only the person removes.",
    instead: "The Store is acted on with `store`: {\"search\": …}, {\"show\": …} or {\"install\": …, \"agent\": …}.",
    closed: "The person closed the Store's tab.",
    keys: "cmd+shift+s",
};

/// What a Store is made of: tests hand it a network, sources and agents of their own.
pub struct Parts {
    pub index: Index,
    /// Where what was fetched is kept: archives, unpacked.
    pub cache: PathBuf,
    pub installed: Installed,
    pub net: Box<dyn Fetch + Send>,
    pub sources: Sources,
    pub targets: Vec<&'static dyn Target>,
    /// The person's machine, when it is known already; else the login shell is asked once.
    pub env: Option<Env>,
    /// What takes time is done at once, on the thread that asked: for tests.
    pub inline: bool,
}

/// What a Store is doing and what it remembers, under one lock.
#[derive(Default)]
struct State {
    installed: Installed,
    /// The sources the person keeps, read into the index.
    sources: Sources,
    tabs: Vec<Weak<dyn Tab>>,
    /// The line under the search field, and whether the sources are being read.
    said: String,
    reading: bool,
    /// How far a read of the sources is: the place of the one being read, of how many.
    at: Option<(usize, usize)>,
    /// Why a source could not be read the last time, by its id.
    failed: BTreeMap<String, String>,
    /// A listing and an agent it is being put into or taken out of, with the words for it.
    doing: BTreeMap<(String, String), String>,
    /// How the last of that ended, until the next is asked.
    ended: BTreeMap<(String, String), String>,
    /// What an agent asked to install, waiting for the person's answer.
    asked: Option<Value>,
    /// The questions up in the window's dialog, by what each is about: none is asked twice.
    asking: BTreeSet<String>,
    /// The last things that happened, for an agent reading the tab.
    lately: Vec<String>,
}

/// The Store.
pub struct Store {
    me: Weak<Store>,
    index: Index,
    cache: PathBuf,
    net: Box<dyn Fetch + Send>,
    targets: Vec<&'static dyn Target>,
    env: OnceLock<Env>,
    inline: bool,
    state: Mutex<State>,
}

/// How many things happened are kept for an agent to read.
const LATELY: usize = 8;

impl Store {
    /// The Store of the app, its files under `data`. An index that does not open is kept in
    /// memory instead: it is read from the sources again.
    pub fn open(data: &Path) -> Result<Arc<Self>, String> {
        let cache = data.join("catalog");
        let index = Index::open(&data.join("catalog.sqlite")).or_else(|why| {
            log::warn!("the Store's index is kept in memory only: {why}");
            Index::memory()
        })?;
        let store = Self::of(Parts {
            index,
            installed: Installed::open(&cache.join("installed.json")),
            sources: Sources::open(&cache.join("sources.json")),
            cache,
            net: Box::new(super::Net),
            targets: super::target::all().to_vec(),
            env: None,
            inline: false,
        });
        store.watch();
        Ok(store)
    }

    pub fn of(parts: Parts) -> Arc<Self> {
        let env = OnceLock::new();
        if let Some(known) = parts.env {
            let _ = env.set(known);
        }
        Arc::new_cyclic(|me| Self {
            me: Weak::clone(me),
            index: parts.index,
            cache: parts.cache,
            net: parts.net,
            targets: parts.targets,
            env,
            inline: parts.inline,
            state: Mutex::new(State {
                installed: parts.installed,
                sources: parts.sources,
                ..State::default()
            }),
        })
    }

    /// The person's machine as their agents see it. The first ask may take seconds.
    fn env(&self) -> &Env {
        self.env.get_or_init(super::target::env)
    }

    fn target(&self, id: &str) -> Result<&'static dyn Target, String> {
        (self
            .targets
            .iter()
            .copied()
            .find(|target| target.id() == id))
        .ok_or_else(|| format!("there is no agent `{id}`"))
    }

    fn listing(&self, id: &str) -> Result<Listing, String> {
        (self.index.get(id)).ok_or_else(|| format!("the Store lists nothing as `{id}`"))
    }

    /// The tab is one of the Store's: what happens is painted in it too.
    fn remember(&self, tab: &Arc<dyn Tab>) {
        let mut state = crate::lock(&self.state);
        state
            .tabs
            .retain(|held| held.upgrade().is_some_and(|held| held.key() != tab.key()));
        state.tabs.push(Arc::downgrade(tab));
    }

    /// `what` with `data`, told to every tab of the Store.
    fn tell(&self, what: &str, data: &Value) {
        let tabs: Vec<_> = (crate::lock(&self.state).tabs.iter())
            .filter_map(Weak::upgrade)
            .collect();
        let frame = page::frame(what, data);
        for tab in tabs {
            tab.paint(&frame);
        }
    }

    /// How many listings there are of each thing the page can show.
    fn counts(&self) -> Value {
        let mut counts = serde_json::Map::new();
        let mut all = 0;
        for (what, count) in self.index.counts() {
            all += count;
            counts.insert(what.name().into(), count.into());
        }
        counts.insert("all".into(), all.into());
        let noted = self.noted();
        let behind = noted.iter().filter(|(_, behind)| *behind).count();
        counts.insert("installed".into(), noted.len().into());
        counts.insert("updates".into(), behind.into());
        Value::Object(counts)
    }

    /// The listings the Store installed that are still listed, each with whether what is
    /// listed now is another version.
    fn noted(&self) -> Vec<(Listing, bool)> {
        let ids = crate::lock(&self.state).installed.ids();
        let listings: Vec<Listing> = ids.iter().filter_map(|id| self.index.get(id)).collect();
        let state = crate::lock(&self.state);
        (listings.into_iter())
            .map(|listing| {
                let behind = state.installed.behind(&listing.id, &listing.version);
                (listing, behind)
            })
            .collect()
    }

    /// Where the Store is: its counts, and its line (empty when there is nothing to say).
    /// `fresh`, the page asks for its list again.
    fn begun(&self, fresh: bool) -> Value {
        let counts = self.counts();
        let state = crate::lock(&self.state);
        json!({
            "counts": counts, "said": state.said, "busy": state.reading, "at": state.at,
            "fresh": fresh,
        })
    }

    /// `what` happened: kept for an agent that reads the tab.
    fn note(&self, what: String) {
        let mut state = crate::lock(&self.state);
        state.lately.push(what);
        let over = state.lately.len().saturating_sub(LATELY);
        state.lately.drain(..over);
    }
}

/// A count as a person reads it: `31,402`.
fn many(count: u64) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// How far each source is trusted and what the person reads it as, by its id.
type Trusts = BTreeMap<String, (u8, &'static str)>;

impl Store {
    /// What reads each source the person keeps.
    fn readers(&self) -> Vec<&'static dyn Source> {
        crate::lock(&self.state).sources.readers()
    }

    fn trusts(&self) -> Trusts {
        (self.readers().iter())
            .map(|source| (source.id().to_string(), (source.trust(), source.title())))
            .collect()
    }

    /// A listing as a card of the page.
    fn row(listing: &Listing, trusts: &Trusts, stand: &Stand) -> Value {
        let (trust, from) = trusts.get(&listing.source).copied().unwrap_or((0, ""));
        json!({
            "id": listing.id, "what": listing.what.name(), "name": listing.name,
            "title": listing.title, "author": listing.author, "summary": listing.summary,
            "category": listing.category, "trust": trust, "from": from,
            "has": stand.has, "update": stand.update, "busy": stand.busy,
        })
    }

    /// Where the Store stands with `listing` now, told to every tab: a card of it is set
    /// right where it lies, and no list is read again.
    fn tell_row(&self, listing: &Listing) {
        let stand = crate::lock(&self.state).stand(listing);
        let row = json!({
            "id": listing.id, "has": stand.has, "update": stand.update, "busy": stand.busy,
        });
        self.tell("row", &row);
    }
}

/// Where the Store stands with a listing: whether it installed it, whether what is listed now
/// is newer, and whether it is being put in or taken out.
struct Stand {
    has: bool,
    update: bool,
    busy: bool,
}

impl State {
    fn stand(&self, listing: &Listing) -> Stand {
        Stand {
            has: self.installed.of(&listing.id).is_some(),
            update: self.installed.behind(&listing.id, &listing.version),
            busy: self.doing.keys().any(|(id, _)| *id == listing.id),
        }
    }
}

impl Kind for Store {
    fn about(&self) -> &'static About {
        &ABOUT
    }

    fn page(&self, title: &str) -> String {
        page::page(title)
    }

    fn loaded(&self, tab: &Arc<dyn Tab>) {
        self.remember(tab);
        tab.paint(&page::frame("begun", &self.begun(true)));
        self.read_sources(false);
    }

    fn call(&self, args: &Value, tab: &Arc<dyn Tab>) -> Result<Said, String> {
        self.remember(tab);
        // An agent may be the first to use the Store, with no page of it ever loaded.
        self.read_sources(false);
        self.asked_by_agent(args, tab)
    }

    fn posted(&self, body: &str, tab: &Arc<dyn Tab>) {
        self.remember(tab);
        self.did(body, tab);
    }

    fn text(&self) -> String {
        self.as_text()
    }

    fn summary(&self) -> String {
        let counts = self.counts();
        let count = |name: &str| many(counts[name].as_u64().unwrap_or(0));
        format!(
            "{} listings, {} installed",
            count("all"),
            count("installed")
        )
    }
}

/// What a listing is, in the page's words.
fn what_word(what: What) -> &'static str {
    match what {
        What::Plugin => "plugin",
        What::Skill => "skill",
        What::Server => "MCP server",
    }
}

#[cfg(test)]
#[path = "kind_tests.rs"]
mod tests;
