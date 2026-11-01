//! Who owns which tab, and what the person said about them.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use super::Call;

/// Where an owner works and how, as its last calls showed: directory, environment, owner.
pub(super) type Place = (PathBuf, Option<Vec<(String, String)>>, String);

/// An owner as its last call showed it.
struct Known {
    cwd: PathBuf,
    vars: Option<Vec<(String, String)>>,
    /// What its tabs are grouped under in the window: its project's name.
    group: String,
}

/// What the person sent about a tab, kept until it is delivered: in a call's answer, by
/// `window/said`, or acknowledged after a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    pub id: u64,
    pub tab: String,
    pub text: String,
}

impl Said {
    /// The `comment/added` notification's params.
    pub fn note(&self, owner: &str) -> serde_json::Value {
        serde_json::json!({"owner": owner, "id": self.id, "tab": self.tab, "text": self.text})
    }
}

#[derive(Default)]
pub(super) struct Registry {
    /// Tab key (`w1`, `t2`) to owner.
    tabs: HashMap<String, String>,
    said: HashMap<String, Vec<Said>>,
    next_said: u64,
    owners: HashMap<String, Known>,
}

/// The name of the project at `cwd`, or `owner` when it is a name an agent chose.
pub(super) fn group(owner: &str) -> String {
    std::path::Path::new(owner).file_name().map_or_else(
        || owner.to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
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
                group: group(call.owner),
            });
        call.cwd.clone_into(&mut entry.cwd);
        if let Some(vars) = call.vars {
            entry.vars = Some(vars.to_vec());
        }
    }

    pub(super) fn group_of(&self, owner: &str) -> String {
        self.owners
            .get(owner)
            .map_or_else(|| group(owner), |known| known.group.clone())
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
        self.said
            .entry(owner.clone())
            .or_default()
            .push(said.clone());
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

    /// The owner whose tabs are grouped under `group`, with its place: where the person's own
    /// terminal ("+") starts.
    pub(super) fn of_group(&self, group: &str) -> Option<Place> {
        let (owner, known) = self.owners.iter().find(|(_, known)| known.group == group)?;
        Some((known.cwd.clone(), known.vars.clone(), owner.clone()))
    }
}
