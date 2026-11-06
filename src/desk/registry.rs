//! Who owns which tab, and what the person said about them.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use super::Call;
use crate::projects::Project;

/// The most notes kept for one owner, until it takes them.
const SAID_KEPT: usize = 200;

/// An owner as its last call showed it: an agent of a project.
struct Known {
    cwd: PathBuf,
    vars: Option<Vec<(String, String)>>,
    project: Project,
    /// Its name within the project; none: the project's own agent.
    agent: Option<String>,
}

/// What the person sent about a tab, kept until it is delivered: in a call's answer, by
/// `window/said`, or acknowledged after a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    pub id: u64,
    pub tab: String,
    pub text: String,
}

#[derive(Default)]
pub(super) struct Registry {
    /// Tab key (`w1`, `t2`) to owner.
    tabs: HashMap<String, String>,
    said: HashMap<String, Vec<Said>>,
    next_said: u64,
    owners: HashMap<String, Known>,
}

impl Registry {
    /// Remember the owner's place and environment.
    pub(super) fn known(&mut self, call: &Call) {
        let entry = self
            .owners
            .entry(call.owner.to_string())
            .or_insert_with(|| Known {
                cwd: call.cwd.to_path_buf(),
                vars: None,
                project: call.project.clone(),
                agent: call.agent.map(str::to_string),
            });
        call.cwd.clone_into(&mut entry.cwd);
        if let Some(vars) = call.vars {
            entry.vars = Some(vars.to_vec());
        }
    }

    /// The window's key of `owner`'s project ("" for an owner not known).
    pub(super) fn group_of(&self, owner: &str) -> String {
        (self.owners.get(owner))
            .map_or_else(String::new, |known| crate::projects::key(known.project.id))
    }

    /// `owner`'s project and its name within it, when it is known.
    pub(super) fn who(&self, owner: &str) -> Option<(&Project, Option<&str>)> {
        let known = self.owners.get(owner)?;
        Some((&known.project, known.agent.as_deref()))
    }

    pub(super) fn own(&mut self, key: &str, owner: &str) {
        self.tabs.insert(key.to_string(), owner.to_string());
    }

    pub(super) fn release(&mut self, key: &str) {
        self.tabs.remove(key);
    }

    /// Every page's key, whoever owns it.
    pub(super) fn pages(&self) -> Vec<String> {
        (self.tabs.keys())
            .filter(|key| key.starts_with('w'))
            .cloned()
            .collect()
    }

    pub(super) fn owner_of(&self, key: &str) -> Option<&str> {
        self.tabs.get(key).map(String::as_str)
    }

    /// `owner`'s tabs, pages first, in the order they were opened.
    pub(super) fn tabs_of(&self, owner: &str) -> Vec<String> {
        let mut keys: Vec<String> = (self.tabs.iter())
            .filter(|(_, tab_owner)| tab_owner.as_str() == owner)
            .map(|(key, _)| key.clone())
            .collect();
        keys.sort_by_key(|key| {
            (
                key.starts_with('t'),
                key.get(1..).and_then(|n| n.parse::<u32>().ok()),
            )
        });
        keys
    }

    pub(super) fn owners(&self) -> BTreeMap<String, Vec<String>> {
        let owners = self.owners.keys().chain(self.tabs.values());
        owners
            .map(|owner| (owner.clone(), self.tabs_of(owner)))
            .collect()
    }

    /// The person said `words` about tab `key`: kept for that tab's owner, who is returned with
    /// what was kept, to be told.
    pub(super) fn hear(&mut self, key: &str, words: &str) -> Option<(String, Said)> {
        let owner = self.tabs.get(key).cloned()?;
        self.next_said += 1;
        let said = Said {
            id: self.next_said,
            tab: key.to_string(),
            text: words.to_string(),
        };
        let kept = self.said.entry(owner.clone()).or_default();
        kept.push(said.clone());
        // An owner that never calls again holds no more than the latest of them.
        let over = kept.len().saturating_sub(SAID_KEPT);
        kept.drain(..over);
        Some((owner, said))
    }

    /// Everything not yet delivered to `owner`, taken: delivering it is taking it.
    pub(super) fn take_said(&mut self, owner: &str) -> Vec<Said> {
        self.said.remove(owner).unwrap_or_default()
    }

    /// What `owner` has not been given yet, left where it is.
    pub(super) fn pending(&self, owner: &str) -> Vec<Said> {
        self.said.get(owner).cloned().unwrap_or_default()
    }

    /// `owner` has what `ids` name: they are not kept any more.
    pub(super) fn acknowledge(&mut self, owner: &str, ids: &[u64]) {
        if let Some(kept) = self.said.get_mut(owner) {
            kept.retain(|said| !ids.contains(&said.id));
        }
    }

    /// The owner of tab `key`, if it has one.
    pub(super) fn owner_named(&self, key: &str) -> Option<String> {
        self.tabs.get(key).cloned()
    }

    pub(super) fn forget(&mut self, owner: &str) {
        self.said.remove(owner);
        self.owners.remove(owner);
    }

    /// The environment an agent of project `id` last called with (its own agent's first): what
    /// the person's own terminal ("+") starts with.
    pub(super) fn vars_of(&self, id: crate::projects::ProjectId) -> Option<Vec<(String, String)>> {
        let mut known: Vec<&Known> = (self.owners.values())
            .filter(|known| known.project.id == id && known.vars.is_some())
            .collect();
        known.sort_by_key(|known| known.agent.is_some());
        known.first().and_then(|known| known.vars.clone())
    }
}
