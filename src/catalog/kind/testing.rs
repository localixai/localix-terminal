//! What the Store's tests are made of: a network that answers nothing, an agent that keeps
//! what it was given, a tab that keeps what was painted in it and answers what it is asked as
//! the test says the person would, and a Store over them.

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::{Parts, Store};
use crate::catalog::target::Target;
use crate::catalog::{
    Batch, Env, Fetch, Fetched, Index, Installed, Listing, Need, Plan, Recipe, Runs, Server, What,
    With,
};
use crate::kind::{Given, Kind as _, Question, Tab};

/// A network no test reaches.
pub(super) struct NoNet;

impl Fetch for NoNet {
    fn text(&self, url: &str, _: Option<&str>) -> Result<Fetched, String> {
        Err(format!("no network in a test: {url}"))
    }

    fn file(&self, url: &str, _: &Path) -> Result<(), String> {
        Err(format!("no network in a test: {url}"))
    }
}

/// An agent that keeps what was put into it: a listing's id, with the values given.
#[derive(Default)]
pub(super) struct Keeps {
    absent: bool,
    pub(super) has: Mutex<BTreeMap<String, BTreeMap<String, String>>>,
}

impl Target for Keeps {
    fn id(&self) -> &'static str {
        "keeps"
    }

    fn name(&self) -> &'static str {
        "Keeps"
    }

    fn present(&self, _: &Env) -> bool {
        !self.absent
    }

    fn takes(&self, listing: &Listing) -> bool {
        listing.what != What::Plugin
    }

    fn plan(&self, listing: &Listing, _: &Env) -> Result<Plan, String> {
        Ok(Plan {
            steps: vec![format!("Add {} to keeps.json", listing.name)],
            after: "Start Keeps again".into(),
        })
    }

    fn install(&self, listing: &Listing, with: &With, _: &Env) -> Result<String, String> {
        crate::lock(&self.has).insert(listing.id.clone(), with.values.clone());
        Ok(format!("Added {} to Keeps.", listing.name))
    }

    fn remove(&self, listing: &Listing, _: &Env) -> Result<String, String> {
        crate::lock(&self.has).remove(&listing.id);
        Ok(format!("Removed {} from Keeps.", listing.name))
    }

    fn has(&self, listing: &Listing, _: &Env) -> Option<String> {
        crate::lock(&self.has)
            .get(&listing.id)
            .map(|_| String::new())
    }
}

/// What a test runs while a question is up.
type During = Box<dyn Fn() + Send + Sync>;

/// A tab that keeps what was painted in it, what it was asked to browse and what it asked the
/// person, who answers as the test said they would: with nothing said, a no.
#[derive(Default)]
pub(super) struct Seen {
    painted: Mutex<Vec<String>>,
    pub(super) browsed: Mutex<Vec<String>>,
    pub(super) asked: Mutex<Vec<Question>>,
    answers: Mutex<VecDeque<Given>>,
    during: Mutex<Option<During>>,
}

impl Tab for Seen {
    fn key(&self) -> &str {
        "p1"
    }

    fn paint(&self, frame: &str) {
        crate::lock(&self.painted).push(frame.to_string());
    }

    fn browse(&self, url: &str) {
        crate::lock(&self.browsed).push(url.to_string());
    }

    fn ask(&self, question: &Question) -> Option<Given> {
        crate::lock(&self.asked).push(question.clone());
        if let Some(during) = crate::lock(&self.during).as_ref() {
            during();
        }
        crate::lock(&self.answers).pop_front()
    }
}

impl Seen {
    /// The person goes on at the next question, giving `given` for its fields.
    pub(super) fn yes(&self, given: &[(&str, &str)]) {
        let given = given
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()));
        crate::lock(&self.answers).push_back(given.collect());
    }

    /// `run` while each question is up.
    pub(super) fn while_asked(&self, run: impl Fn() + Send + Sync + 'static) {
        *crate::lock(&self.during) = Some(Box::new(run));
    }

    /// The last question asked.
    pub(super) fn question(&self) -> Question {
        let asked = crate::lock(&self.asked);
        asked
            .last()
            .cloned()
            .unwrap_or_else(|| panic!("nothing was asked"))
    }

    /// The data of the last `what` painted.
    pub(super) fn last(&self, what: &str) -> Value {
        let head = format!("S(\"{what}\",");
        let painted = crate::lock(&self.painted);
        let frame = painted.iter().rev().find(|frame| frame.starts_with(&head));
        let frame = frame.unwrap_or_else(|| panic!("no `{what}` was painted: {painted:?}"));
        serde_json::from_str(
            frame
                .strip_prefix(&head)
                .unwrap()
                .strip_suffix(')')
                .unwrap(),
        )
        .unwrap()
    }
}

pub(super) fn server(name: &str, version: &str, needs: Vec<Need>) -> Listing {
    Listing {
        id: format!("reg/{name}"),
        what: What::Server,
        source: "reg".into(),
        name: name.into(),
        title: name.to_uppercase(),
        summary: format!("{name} over MCP"),
        author: "acme".into(),
        category: String::new(),
        homepage: format!("https://example.com/{name}"),
        version: version.into(),
        origin: String::new(),
        recipe: Recipe::Server(Server {
            runs: Runs::Remote {
                url: format!("https://example.com/{name}/mcp"),
                sse: false,
            },
            needs,
        }),
    }
}

pub(super) fn key_need() -> Need {
    Need {
        name: "API_KEY".into(),
        about: "The key".into(),
        required: true,
        secret: true,
        header: true,
        default: String::new(),
    }
}

/// A Store listing `listings`, its agent, and a tab of it with its page loaded.
pub(super) fn store(
    listings: Vec<Listing>,
) -> (Arc<Store>, &'static Keeps, Arc<Seen>, Arc<dyn Tab>) {
    let index = Index::memory().unwrap();
    let batch = Batch {
        listings,
        whole: true,
        ..Batch::default()
    };
    index.put("reg", 1, &batch, 1).unwrap();
    let keeps: &'static Keeps = Box::leak(Box::default());
    let store = Store::of(Parts {
        index,
        cache: std::env::temp_dir().join("llxterm-no-cache"),
        installed: Installed::default(),
        net: Box::new(NoNet),
        sources: crate::catalog::sources::Sources::default(),
        targets: vec![keeps],
        env: Some(Env::default()),
        inline: true,
    });
    let seen = Arc::new(Seen::default());
    let tab: Arc<dyn Tab> = Arc::clone(&seen) as _;
    store.loaded(&tab);
    (store, keeps, seen, tab)
}

/// A source that lists what a test gave it, whole each time it is read.
pub(super) struct Lists {
    pub(super) id: &'static str,
    pub(super) trust: u8,
    pub(super) listings: Mutex<Vec<Listing>>,
}

impl crate::catalog::Source for Lists {
    fn id(&self) -> &'static str {
        self.id
    }

    fn title(&self) -> &'static str {
        "Listed"
    }

    fn trust(&self) -> u8 {
        self.trust
    }

    fn read(&self, _: &dyn Fetch, _: Option<&str>, _: &Path) -> Result<Option<Batch>, String> {
        Ok(Some(Batch {
            listings: crate::lock(&self.listings).clone(),
            whole: true,
            ..Batch::default()
        }))
    }
}

/// A Store that reads the source `reg`, trusted as far as `trust`, which lists `listings`: with
/// its agent, a tab of it loaded, and the source, to list something else later.
pub(super) fn sourced(
    trust: u8,
    listings: Vec<Listing>,
) -> (
    Arc<Store>,
    &'static Keeps,
    Arc<Seen>,
    Arc<dyn Tab>,
    &'static Lists,
) {
    let lists: &'static Lists = Box::leak(Box::new(Lists {
        id: "reg",
        trust,
        listings: Mutex::new(listings),
    }));
    let keeps: &'static Keeps = Box::leak(Box::default());
    let store = Store::of(Parts {
        index: Index::memory().unwrap(),
        cache: std::env::temp_dir().join("llxterm-no-cache"),
        installed: Installed::default(),
        net: Box::new(NoNet),
        sources: crate::catalog::sources::Sources::of(vec![lists]),
        targets: vec![keeps],
        env: Some(Env::default()),
        inline: true,
    });
    let seen = Arc::new(Seen::default());
    let tab: Arc<dyn Tab> = Arc::clone(&seen) as _;
    store.loaded(&tab);
    (store, keeps, seen, tab, lists)
}

pub(super) fn post(store: &Store, tab: &Arc<dyn Tab>, action: &Value) {
    store.posted(&action.to_string(), tab);
}

/// Listing `id` installed in the test's agent as a person does it: Install in the page, and a
/// yes in the dialog with `given` typed.
pub(super) fn install(
    store: &Store,
    seen: &Seen,
    tab: &Arc<dyn Tab>,
    id: &str,
    given: &[(&str, &str)],
) {
    seen.yes(given);
    post(
        store,
        tab,
        &serde_json::json!({"do": "plan", "id": id, "agent": "keeps"}),
    );
}
