//! What takes the Store time: reading the sources into the index, and putting a listing into
//! an agent or taking it out. Each is done on a thread of its own, says how far it is in the
//! Store's tabs, and ends by saying what is true now.

use std::sync::Arc;

use serde_json::{Value, json};

use super::{Store, many};
use crate::catalog::{Listing, Query, Recipe, With, archive, index, target::Target};

/// How many similar listings a listing's own view offers.
const LIKE: usize = 6;

impl Store {
    /// `work` done off the thread that asked for it.
    pub(super) fn go(&self, name: &str, work: impl FnOnce(Arc<Store>) + Send + 'static) {
        let Some(me) = self.me.upgrade() else { return };
        if self.inline {
            return work(me);
        }
        let spawned = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || work(me));
        if let Err(why) = spawned {
            log::warn!("the Store could not start {name}: {why}");
        }
    }

    /// The sources that are due read into the index (every one when `forced`), unless a read
    /// is under way.
    pub(super) fn read_sources(&self, forced: bool) {
        self.read_some(forced, None);
    }

    /// As [`Store::read_sources`], of source `only` alone when one is named.
    pub(super) fn read_some(&self, forced: bool, only: Option<String>) {
        {
            let mut state = crate::lock(&self.state);
            if state.reading {
                return;
            }
            state.reading = true;
        }
        self.go("store-sources", move |store| {
            store.read_now(forced, only.as_deref());
        });
    }

    fn read_now(&self, forced: bool, only: Option<&str>) {
        // Each step is told with the counts as they are now; a source's first listings are
        // shown at once, while the longer sources are still read.
        let mut sources = self.readers();
        sources.retain(|source| only.is_none_or(|only| source.id() == only));
        let sources = &sources;
        let shown = std::sync::Mutex::new(Vec::new());
        let told = |progress: &index::Progress| {
            let first = progress.listings > 0 && {
                let mut shown = crate::lock(&shown);
                let first = !shown.contains(&progress.source);
                shown.push(progress.source);
                first
            };
            let said = format!("Reading {}… {}", progress.title, many(progress.listings));
            let at = sources
                .iter()
                .position(|source| source.id() == progress.source);
            {
                let mut state = crate::lock(&self.state);
                state.said = said;
                state.at = at.map(|at| (at, sources.len()));
            }
            self.tell("begun", &self.begun(first));
        };
        let now = crate::now();
        let read = index::refresh(
            &self.index,
            sources,
            &*self.net,
            &self.cache,
            now,
            forced,
            &told,
        );
        let wrote: u64 = read
            .iter()
            .filter_map(|(_, wrote)| wrote.as_ref().ok())
            .sum();
        let failed: Vec<String> = (read.iter())
            .filter_map(|(id, wrote)| {
                let why = wrote.as_ref().err()?;
                log::warn!("the Store could not read {id}: {why}");
                let title = sources.iter().find(|source| source.id() == *id)?.title();
                Some(title.to_string())
            })
            .collect();
        {
            let mut state = crate::lock(&self.state);
            for (id, wrote) in &read {
                match wrote {
                    Ok(_) => state.failed.remove(*id),
                    Err(why) => state.failed.insert((*id).to_string(), why.clone()),
                };
            }
            state.reading = false;
            state.at = None;
            state.said = match failed.is_empty() {
                true => String::new(),
                false => format!(
                    "{} could not be read; what was listed before is shown",
                    failed.join(", ")
                ),
            };
        }
        self.tell("begun", &self.begun(wrote > 0));
        self.tell("sources", &self.sources());
    }

    /// The folder of a skill, fetched and unpacked: what an install of it copies.
    fn folder_of(&self, listing: &Listing) -> Result<Option<std::path::PathBuf>, String> {
        let Recipe::Skill { repo, rev, path } = &listing.recipe else {
            return Ok(None);
        };
        let plain = path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
        if !plain || path.starts_with('/') {
            return Err(format!("`{path}` is no folder of {repo}"));
        }
        let folder = archive::unpacked(&*self.net, &self.cache, repo, rev)?.join(path);
        match folder.join("SKILL.md").is_file() {
            true => Ok(Some(folder)),
            false => Err(format!("{repo} has no skill at {path} any more")),
        }
    }

    /// `listing` is being put into `target` with the person's `values`, or taken out of it
    /// (`values` none): said in the tabs now, and done on a thread of its own.
    pub(super) fn change(
        &self,
        listing: Listing,
        target: &'static dyn Target,
        values: Option<std::collections::BTreeMap<String, String>>,
    ) {
        let pair = (listing.id.clone(), target.id().to_string());
        {
            let mut state = crate::lock(&self.state);
            if state.doing.contains_key(&pair) {
                return;
            }
            let word = if values.is_some() {
                "Installing…"
            } else {
                "Removing…"
            };
            state.doing.insert(pair.clone(), word.into());
            state.ended.remove(&pair);
        }
        self.tell("shown", &self.shown(&listing));
        self.tell_row(&listing);
        self.go("store-install", move |store| {
            let done = match values {
                Some(values) => store.folder_of(&listing).and_then(|folder| {
                    let with = With { values, folder };
                    target.install(&listing, &with, store.env())
                }),
                None => target.remove(&listing, store.env()),
            };
            if done.is_ok() {
                store.noted_in(&listing, target);
            }
            let (said, noted) = match &done {
                Ok(said) => (said.clone(), format!("{}: {said}", listing.id)),
                Err(why) => (
                    why.clone(),
                    format!("{} was not changed in {}: {why}", listing.id, target.name()),
                ),
            };
            {
                let mut state = crate::lock(&store.state);
                state.doing.remove(&pair);
                state.ended.insert(pair, said);
            }
            store.note(noted);
            store.tell("shown", &store.shown(&listing));
            store.tell_row(&listing);
            store.tell("begun", &store.begun(false));
        });
    }

    /// `listing` as the page shows it picked: what it is, and where each agent stands with it.
    /// The Store's note of what it installed is set right against the agents' own files.
    pub(super) fn shown(&self, listing: &Listing) -> Value {
        let env = self.env();
        let agents: Vec<Value> = (self.targets.iter())
            .map(|target| {
                let takes = target.takes(listing);
                let present = target.present(env);
                let has = if takes {
                    target.has(listing, env)
                } else {
                    None
                };
                let pair = (listing.id.clone(), target.id().to_string());
                let mut state = crate::lock(&self.state);
                let noted = (state.installed.of(&listing.id))
                    .and_then(|agents| agents.get(target.id()).cloned());
                match (&has, &noted) {
                    (None, Some(_)) => state.installed.set(&listing.id, target.id(), None),
                    // Put there by another hand: its version is not the Store's to compare.
                    (Some(_), None) => state.installed.set(&listing.id, target.id(), Some("")),
                    (None, None) | (Some(_), Some(_)) => {}
                }
                let had = noted.unwrap_or_default();
                let update = has.is_some()
                    && !had.is_empty()
                    && !listing.version.is_empty()
                    && had != listing.version;
                json!({
                    "id": target.id(), "name": target.name(), "present": present, "takes": takes,
                    "has": has, "update": update,
                    "busy": state.doing.get(&pair), "said": state.ended.get(&pair),
                })
            })
            .collect();
        let trusts = self.trusts();
        let (trust, source) = trusts.get(&listing.source).copied().unwrap_or((0, ""));
        let also: Vec<&str> = (self.index.also_in(&listing.id).iter())
            .filter_map(|id| Some(trusts.get(id)?.1))
            .collect();
        json!({
            "listing": {
                "id": listing.id, "what": listing.what.name(), "name": listing.name,
                "title": listing.title, "author": listing.author, "summary": listing.summary,
                "homepage": listing.homepage, "version": listing.version,
                "category": listing.category, "trust": trust, "origin": listing.origin,
            },
            "source": source, "also": also, "runs_code": listing.runs_code(), "agents": agents,
            "more": self.like(listing, &trusts),
        })
    }

    /// A few other listings of `listing`'s kind and category, as cards: what a person who
    /// opened it may want next.
    fn like(&self, listing: &Listing, trusts: &super::Trusts) -> Vec<Value> {
        let found = self.index.search(&Query {
            what: Some(listing.what),
            category: Some(listing.category.clone()),
            limit: LIKE + 1,
            ..Query::default()
        });
        let found = found.map(|found| found.listings).unwrap_or_default();
        let state = crate::lock(&self.state);
        (found.iter())
            .filter(|other| other.id != listing.id)
            .take(LIKE)
            .map(|other| Self::row(other, trusts, &state.stand(other)))
            .collect()
    }

    /// `listing` is in `target` now, at the version listed: noted, so that a newer one shows.
    pub(super) fn noted_in(&self, listing: &Listing, target: &dyn Target) {
        let has = target.has(listing, self.env());
        let version = has.as_ref().map(|_| listing.version.as_str());
        crate::lock(&self.state)
            .installed
            .set(&listing.id, target.id(), version);
    }
}
