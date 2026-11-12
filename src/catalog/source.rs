//! Where the catalogue comes from: the port a source is read through, and the sources. A source
//! reads what it lists through [`Fetch`] (the network, faked in a test) and says what is new
//! since the mark it left last time: a [`Batch`]. It keeps nothing itself; the index keeps the
//! listings and the mark.

mod frontmatter;
mod marketplace;
mod origin;
mod registry;
mod skills;

use std::path::Path;
use std::time::Duration;

use super::model::{Listing, PluginFormat};

pub use crate::fetch::Fetched;
pub use marketplace::Marketplace;
pub use registry::Registry;
pub use skills::SkillsRepo;

/// The network as a source reads it.
pub trait Fetch: Sync {
    /// What `url` answers as text; [`Fetched::Same`] when it is still what `etag` was of.
    fn text(&self, url: &str, etag: Option<&str>) -> Result<Fetched, String>;
    /// `url` written to the file `to`.
    fn file(&self, url: &str, to: &Path) -> Result<(), String>;
}

/// The network itself, by the system's `curl`.
pub struct Net;

/// How long a short answer, and a file, may take.
const TEXT_WITHIN: Duration = Duration::from_secs(60);
const FILE_WITHIN: Duration = Duration::from_secs(10 * 60);

/// The longest answer a source reads, in bytes: a marketplace's file lists thousands of
/// plugins, and grows.
pub const TEXT_MOST: usize = 16 * 1024 * 1024;

impl Fetch for Net {
    fn text(&self, url: &str, etag: Option<&str>) -> Result<Fetched, String> {
        let ask = crate::fetch::Ask {
            etag,
            most: TEXT_MOST,
            ..crate::fetch::Ask::within(TEXT_WITHIN)
        };
        let dir = crate::paths::data().join("catalog").join("fetching");
        crate::fetch::changed(url, &dir, &ask)
    }

    fn file(&self, url: &str, to: &Path) -> Result<(), String> {
        crate::fetch::file(url, to, FILE_WITHIN, &|_| {})
    }
}

/// What a source read once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Batch {
    pub listings: Vec<Listing>,
    /// The ids of listings the source no longer has.
    pub gone: Vec<String>,
    /// What the source reads on from next time: an ETag, where a pass through it is.
    pub mark: String,
    /// The listings are all the source has: what it listed before and not now is gone.
    pub whole: bool,
    /// There is more of this pass: the source is read again now, from `mark`.
    pub more: bool,
}

/// One place listings come from.
pub trait Source: Sync {
    /// Its name among sources: stable, in the ids of its listings.
    fn id(&self) -> &'static str;
    /// What the person reads it as.
    fn title(&self) -> &'static str;
    /// How far it is trusted, ranking its listings: a curated list above an open registry.
    fn trust(&self) -> u8;
    /// What is new since `mark`, the one its last batch left; none when nothing is. `cache` is
    /// a folder it may keep what it fetched in.
    fn read(
        &self,
        net: &dyn Fetch,
        mark: Option<&str>,
        cache: &Path,
    ) -> Result<Option<Batch>, String>;
}

/// The marketplace Claude Code's makers keep.
static CLAUDE_PLUGINS: Marketplace = Marketplace {
    id: "claude-plugins",
    title: "Claude Plugins",
    repo: "anthropics/claude-plugins-official",
    trust: 3,
    format: PluginFormat::Claude,
};

/// The marketplace of plugins anyone made, for Claude Code.
static CLAUDE_COMMUNITY: Marketplace = Marketplace {
    id: "claude-community",
    title: "Claude Community",
    repo: "anthropics/claude-plugins-community",
    trust: 2,
    format: PluginFormat::Claude,
};

/// The marketplace Codex's makers keep.
static CODEX_PLUGINS: Marketplace = Marketplace {
    id: "codex-plugins",
    title: "Codex Plugins",
    repo: "openai/plugins",
    trust: 3,
    format: PluginFormat::Codex,
};

static SKILLS: SkillsRepo = SkillsRepo {
    id: "skills",
    title: "Agent Skills",
    repo: "anthropics/skills",
    trust: 3,
};

/// The skills Codex's makers keep.
static CODEX_SKILLS: SkillsRepo = SkillsRepo {
    id: "codex-skills",
    title: "Codex Skills",
    repo: "openai/skills",
    trust: 3,
};

static HF_SKILLS: SkillsRepo = SkillsRepo {
    id: "hf-skills",
    title: "Hugging Face Skills",
    repo: "huggingface/skills",
    trust: 2,
};

static VERCEL_SKILLS: SkillsRepo = SkillsRepo {
    id: "vercel-skills",
    title: "Vercel Skills",
    repo: "vercel-labs/agent-skills",
    trust: 2,
};

static REGISTRY: Registry = Registry { now: crate::now };

/// The sources the app knows by itself, in the order they are read: those a vendor keeps
/// (trust 3), then those that list anyone's work (2), then the open registry (1).
pub fn all() -> &'static [&'static dyn Source] {
    static ALL: [&dyn Source; 8] = [
        &CLAUDE_PLUGINS,
        &CLAUDE_COMMUNITY,
        &CODEX_PLUGINS,
        &SKILLS,
        &CODEX_SKILLS,
        &HF_SKILLS,
        &VERCEL_SKILLS,
        &REGISTRY,
    ];
    &ALL
}

/// [`all`], as a list of its own.
pub fn defaults() -> Vec<&'static dyn Source> {
    all().to_vec()
}

/// `text` kept for as long as the program runs.
fn kept(text: &str) -> &'static str {
    Box::leak(text.to_owned().into_boxed_str())
}

/// A marketplace made from what the person configured. It is kept for as long as the program
/// runs: make each one once, and keep what this gives.
pub fn marketplace(
    id: &str,
    title: &str,
    repo: &str,
    trust: u8,
    format: PluginFormat,
) -> &'static dyn Source {
    Box::leak(Box::new(Marketplace {
        id: kept(id),
        title: kept(title),
        repo: kept(repo),
        trust,
        format,
    }))
}

/// A repository of skills made from what the person configured, kept as [`marketplace`] is.
pub fn skills_repo(id: &str, title: &str, repo: &str, trust: u8) -> &'static dyn Source {
    Box::leak(Box::new(SkillsRepo {
        id: kept(id),
        title: kept(title),
        repo: kept(repo),
        trust,
    }))
}

/// Whether `text` names a GitHub repository as `owner/repo`: two parts of 1 to 100 letters,
/// digits, `_`, `.` or `-`, neither starting with a dot nor holding `..`.
pub fn repo_ok(text: &str) -> bool {
    let part = |part: &str| {
        (1..=100).contains(&part.len())
            && !part.starts_with('.')
            && !part.contains("..")
            && (part.chars()).all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    };
    text.split_once('/')
        .is_some_and(|(owner, name)| part(owner) && part(name))
}

/// A name as words a person reads: `google-drive` as "Google Drive".
pub(super) fn readable(name: &str) -> String {
    let words = name.split(['-', '_', ' ']).filter(|word| !word.is_empty());
    let words: Vec<String> = words.map(super::capital).collect();
    words.join(" ")
}

/// The text at `key` of a JSON object, empty where it is none or not text.
pub(super) fn text_at(value: &serde_json::Value, key: &str) -> String {
    (value.get(key).and_then(serde_json::Value::as_str))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// The first twelve characters of a commit, as its short name.
pub(super) fn short(commit: &str) -> String {
    commit.chars().take(12).collect()
}

#[cfg(test)]
#[path = "source_tests.rs"]
pub(crate) mod tests;
