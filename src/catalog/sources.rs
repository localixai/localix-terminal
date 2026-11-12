//! The sources the person's Store reads, as they keep them: a list in a file of its own. It
//! begins with the ones the app trusts, so that a new Store is not empty; each can be removed,
//! and one removed is not put back until the person asks. A source they add is a GitHub
//! repository that holds a plugin marketplace or agent skills. Each source says whether what
//! was installed from it is kept up to date without asking.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::PluginFormat;
use super::source::{self, Source};

/// What a source holds, which is how it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// A plugin marketplace as Claude Code reads one.
    Claude,
    /// A plugin marketplace as Codex reads one.
    Codex,
    /// A repository of agent skills.
    Skills,
    /// The open registry of MCP servers.
    Registry,
}

/// One source as the file keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    /// Where it is: `owner/repo` on GitHub, or the registry's host.
    pub repo: String,
    pub shape: Shape,
    /// It is one the app begins with.
    #[serde(default)]
    pub stock: bool,
}

/// A source of the list: how it is kept, and what reads it.
#[derive(Clone)]
pub struct Kept {
    pub entry: Entry,
    pub source: &'static dyn Source,
}

/// The file's content.
#[derive(Default, Serialize, Deserialize)]
struct Written {
    #[serde(default)]
    sources: Vec<Entry>,
    /// The ids of the app's own sources the person removed.
    #[serde(default)]
    removed: BTreeSet<String>,
}

/// The sources the app begins with, the ones a vendor keeps first.
const STOCK: [(&str, &str, Shape); 8] = [
    (
        "claude-plugins",
        "anthropics/claude-plugins-official",
        Shape::Claude,
    ),
    (
        "claude-community",
        "anthropics/claude-plugins-community",
        Shape::Claude,
    ),
    ("codex-plugins", "openai/plugins", Shape::Codex),
    ("skills", "anthropics/skills", Shape::Skills),
    ("codex-skills", "openai/skills", Shape::Skills),
    ("hf-skills", "huggingface/skills", Shape::Skills),
    ("vercel-skills", "vercel-labs/agent-skills", Shape::Skills),
    (
        "mcp-registry",
        "registry.modelcontextprotocol.io",
        Shape::Registry,
    ),
];

/// How far a source the person added is trusted: as a list of anyone's work.
const ADDED_TRUST: u8 = 2;

/// The list.
#[derive(Default)]
pub struct Sources {
    file: Option<PathBuf>,
    held: Vec<Kept>,
    removed: BTreeSet<String>,
}

impl Sources {
    /// The list kept in `file`: the app's own sources that were not removed, then the person's.
    pub fn open(file: &Path) -> Self {
        let written: Written = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let mut sources = Self {
            file: Some(file.to_path_buf()),
            held: Vec::new(),
            removed: written.removed,
        };
        let stock = source::defaults();
        for (id, repo, shape) in STOCK {
            let Some(source) = stock.iter().copied().find(|source| source.id() == id) else {
                continue;
            };
            if sources.removed.contains(id) {
                continue;
            }
            sources.held.push(Kept {
                entry: Entry {
                    id: id.into(),
                    repo: repo.into(),
                    shape,
                    stock: true,
                },
                source,
            });
        }
        for entry in written.sources {
            let known = sources.held.iter().any(|kept| kept.entry.id == entry.id);
            let is_stock = STOCK.iter().any(|(id, ..)| *id == entry.id);
            if known || is_stock {
                continue;
            }
            if let Some(source) = made(&entry.id, &entry.repo, entry.shape) {
                let entry = Entry {
                    stock: false,
                    ..entry
                };
                sources.held.push(Kept { entry, source });
            }
        }
        sources
    }

    /// A list of `sources` kept nowhere, for a Store made of parts.
    pub fn of(sources: Vec<&'static dyn Source>) -> Self {
        let held = (sources.into_iter())
            .map(|source| Kept {
                entry: Entry {
                    id: source.id().into(),
                    repo: String::new(),
                    shape: Shape::Claude,
                    stock: false,
                },
                source,
            })
            .collect();
        Self {
            file: None,
            held,
            removed: BTreeSet::new(),
        }
    }

    pub fn list(&self) -> &[Kept] {
        &self.held
    }

    /// What reads each source, in the list's order.
    pub fn readers(&self) -> Vec<&'static dyn Source> {
        self.held.iter().map(|kept| kept.source).collect()
    }

    /// The source `id`.
    pub fn get(&self, id: &str) -> Option<&Kept> {
        self.held.iter().find(|kept| kept.entry.id == id)
    }

    /// How many of the app's own sources the person removed.
    pub fn removed(&self) -> usize {
        self.removed.len()
    }

    /// The source that reads `repo`, if the list has it.
    pub fn of_repo(&self, repo: &str) -> Option<&Kept> {
        (self.held.iter()).find(|kept| kept.entry.repo.eq_ignore_ascii_case(repo))
    }

    /// `repo`, found to hold `shape`, joins the list, read by `source`.
    pub fn add(&mut self, repo: &str, shape: Shape, source: &'static dyn Source) {
        if self.get(source.id()).is_some() {
            return;
        }
        self.held.push(Kept {
            entry: Entry {
                id: source.id().into(),
                repo: repo.into(),
                shape,
                stock: false,
            },
            source,
        });
        self.save();
    }

    /// Source `id` leaves the list; whether it was there. One of the app's own is remembered
    /// as removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let Some(at) = self.held.iter().position(|kept| kept.entry.id == id) else {
            return false;
        };
        if self.held.remove(at).entry.stock {
            self.removed.insert(id.to_string());
        }
        self.save();
        true
    }

    /// The app's own sources the person removed are in the list again, in their places.
    pub fn restore(&mut self) {
        if self.removed.is_empty() {
            return;
        }
        self.removed.clear();
        self.save();
        let Some(file) = self.file.clone() else {
            return;
        };
        *self = Self::open(&file);
    }

    /// The list written whole beside its file, then put in its place.
    fn save(&self) {
        let Some(file) = &self.file else { return };
        let written = Written {
            sources: self.held.iter().map(|kept| kept.entry.clone()).collect(),
            removed: self.removed.clone(),
        };
        let done = serde_json::to_string_pretty(&written)
            .map_err(|why| why.to_string())
            .and_then(|text| crate::setup::reach::write_over(file, &text));
        if let Err(why) = done {
            log::warn!("the Store's list of sources was not kept: {why}");
        }
    }
}

/// What reads `repo` as `shape`, under the name `id`; none for what a person cannot add.
fn made(id: &str, repo: &str, shape: Shape) -> Option<&'static dyn Source> {
    if !source::repo_ok(repo) {
        return None;
    }
    match shape {
        Shape::Claude => Some(source::marketplace(
            id,
            repo,
            repo,
            ADDED_TRUST,
            PluginFormat::Claude,
        )),
        Shape::Codex => Some(source::marketplace(
            id,
            repo,
            repo,
            ADDED_TRUST,
            PluginFormat::Codex,
        )),
        Shape::Skills => Some(source::skills_repo(id, repo, repo, ADDED_TRUST)),
        Shape::Registry => None,
    }
}

/// The repository a person means by `text`: `owner/repo`, or its address on GitHub.
pub fn repo_of(text: &str) -> Option<String> {
    let text = text.trim().trim_end_matches('/');
    let text = text.strip_suffix(".git").unwrap_or(text);
    let text = ["https://", "http://", "git@"]
        .iter()
        .find_map(|start| text.strip_prefix(start))
        .unwrap_or(text);
    let text = ["github.com/", "github.com:", "www.github.com/"]
        .iter()
        .find_map(|host| text.strip_prefix(host))
        .unwrap_or(text);
    let mut parts = text.split('/');
    let repo = format!("{}/{}", parts.next()?, parts.next()?);
    source::repo_ok(&repo).then_some(repo)
}

/// The name a source of `repo` has among sources: `owner-repo`, in lower case.
pub fn id_of(repo: &str) -> String {
    repo.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Every way `repo` might be read, the likeliest first: the one that finds listings is its
/// shape.
pub fn ways(repo: &str) -> Vec<(Shape, &'static dyn Source)> {
    let id = id_of(repo);
    [Shape::Claude, Shape::Codex, Shape::Skills]
        .into_iter()
        .filter_map(|shape| Some((shape, made(&id, repo, shape)?)))
        .collect()
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
