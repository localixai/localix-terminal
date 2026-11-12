//! What the Store installed, as it remembers it: each listing's id, and under it the agents it
//! was put into with the version put. It is a note, not the truth: the agents' own files are,
//! and the note is set right against them whenever a listing is shown.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The note, and the file it is kept in.
#[derive(Debug, Default)]
pub struct Installed {
    file: Option<PathBuf>,
    held: BTreeMap<String, BTreeMap<String, String>>,
}

impl Installed {
    /// The note kept in `file`; empty when there is none yet, or it does not read.
    pub fn open(file: &Path) -> Self {
        let held = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            file: Some(file.to_path_buf()),
            held,
        }
    }

    /// Listing `id` is in `agent` at `version`, or with none is not there any more. Written
    /// out when that is news.
    pub fn set(&mut self, id: &str, agent: &str, version: Option<&str>) {
        let before = self.held.get(id).and_then(|agents| agents.get(agent));
        if before.map(String::as_str) == version {
            return;
        }
        match version {
            Some(version) => {
                let agents = self.held.entry(id.to_string()).or_default();
                agents.insert(agent.to_string(), version.to_string());
            }
            None => {
                if let Some(agents) = self.held.get_mut(id) {
                    agents.remove(agent);
                    if agents.is_empty() {
                        self.held.remove(id);
                    }
                }
            }
        }
        self.save();
    }

    /// The agents listing `id` is in, each with the version put there.
    pub fn of(&self, id: &str) -> Option<&BTreeMap<String, String>> {
        self.held.get(id)
    }

    /// Every listing noted, by its id.
    pub fn ids(&self) -> Vec<String> {
        self.held.keys().cloned().collect()
    }

    /// Whether listing `id`, now at `version`, is in an agent at another one.
    pub fn behind(&self, id: &str, version: &str) -> bool {
        !version.is_empty()
            && self
                .of(id)
                .is_some_and(|agents| agents.values().any(|had| !had.is_empty() && had != version))
    }

    /// The note written whole beside its file, then put in its place.
    fn save(&self) {
        let Some(file) = &self.file else { return };
        let written = serde_json::to_string_pretty(&self.held)
            .map_err(|why| why.to_string())
            .and_then(|text| crate::setup::reach::write_over(file, &text));
        if let Err(why) = written {
            log::warn!("the Store's note of what it installed was not kept: {why}");
        }
    }
}

#[cfg(test)]
#[path = "installed_tests.rs"]
mod tests;
