//! The catalogue's own words, which name no source and no agent: a listing (a plugin, a skill
//! or an MCP server someone published), the recipe it is installed by, and what an install asks
//! of the person first. A source (source.rs) turns what it reads into these; a target
//! (target.rs) turns these into what one agent keeps.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What a listing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum What {
    /// A package an agent's own program installs: skills, commands, hooks and servers together.
    Plugin,
    /// A folder with a `SKILL.md`: instructions an agent reads when they apply.
    Skill,
    /// An MCP server: a program started, or an address called, for the tools it gives.
    Server,
}

impl What {
    pub const ALL: [Self; 3] = [Self::Plugin, Self::Skill, Self::Server];

    /// Its name, as the index keeps it and a call gives it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Plugin => "plugin",
            Self::Skill => "skill",
            Self::Server => "server",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|what| what.name() == name)
    }
}

/// One thing a source lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    /// Its name among all sources: `<source>/<name>`.
    pub id: String,
    pub what: What,
    /// The source that lists it (its id).
    pub source: String,
    /// Its name as its source and the agents know it.
    pub name: String,
    /// What it is called where a person reads it.
    pub title: String,
    /// What it does, in its publisher's words.
    pub summary: String,
    pub author: String,
    pub category: String,
    /// Where to read more: a web address, or none.
    pub homepage: String,
    /// The version or revision its source pins, empty when it pins none. Two listings of the
    /// same id differ in it exactly when there is something new to install.
    pub version: String,
    /// Where the thing itself lives, one name for it whichever source lists it: a repository
    /// as `github.com/owner/repo`, with `@ref` for a branch or tag it is taken at and `#path`
    /// for a folder in it. Empty when it is not known; two listings of one kind and one origin
    /// are the same thing, listed twice.
    #[serde(default)]
    pub origin: String,
    pub recipe: Recipe,
}

/// How a listing is installed, in words no agent owns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "lowercase")]
pub enum Recipe {
    /// By an agent's own program, from a marketplace it is told of.
    Plugin {
        /// The marketplace's name, as its file gives it.
        marketplace: String,
        /// Where the marketplace is added from: `owner/repo` on GitHub.
        from: String,
        format: PluginFormat,
    },
    /// A folder of a repository's archive, copied to where an agent reads skills.
    Skill {
        /// `owner/repo` on GitHub.
        repo: String,
        /// The branch, tag or commit the folder is taken at.
        rev: String,
        /// The folder in the repository: `skills/pdf`.
        path: String,
    },
    /// An entry in an agent's list of MCP servers.
    Server(Server),
}

/// Whose marketplace file lists a plugin: the agents that read that file can install it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginFormat {
    /// `.claude-plugin/marketplace.json`.
    Claude,
    /// `.agents/plugins/marketplace.json`.
    Codex,
}

/// An MCP server as any agent can be told of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub runs: Runs,
    /// What it needs before it works: keys, tokens, settings.
    pub needs: Vec<Need>,
}

/// How an MCP server is reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "as", rename_all = "lowercase")]
pub enum Runs {
    /// A program on the person's machine, spoken to over its standard streams. It runs code.
    Command { command: String, args: Vec<String> },
    /// An address called over HTTP: streamable, or the older server-sent events when `sse`.
    Remote { url: String, sse: bool },
}

/// A value an MCP server needs from the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Need {
    /// The variable's name, or the header's.
    pub name: String,
    /// What it is for, in its publisher's words.
    pub about: String,
    pub required: bool,
    /// A key or a token: never shown again once given, never logged.
    pub secret: bool,
    /// Sent as a header of a remote server; else set in a program's environment.
    pub header: bool,
    /// What it is when the person gives nothing.
    pub default: String,
}

impl Listing {
    /// Whether installing it puts code on the person's machine that runs unasked: a plugin may
    /// carry hooks and servers, a server that is a program is one. A skill is text an agent
    /// reads, and a remote server runs elsewhere.
    pub fn runs_code(&self) -> bool {
        match &self.recipe {
            Recipe::Plugin { .. } => true,
            Recipe::Skill { .. } => false,
            Recipe::Server(server) => matches!(server.runs, Runs::Command { .. }),
        }
    }

    /// What the person must give before it is installed.
    pub fn needs(&self) -> &[Need] {
        match &self.recipe {
            Recipe::Server(server) => &server.needs,
            Recipe::Plugin { .. } | Recipe::Skill { .. } => &[],
        }
    }
}

/// The person's machine as an agent's programs see it: their home, the `PATH` and variables of
/// their login shell (never this process's own, which the Finder gives almost none).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Env {
    pub home: PathBuf,
    pub path: String,
    pub vars: BTreeMap<String, String>,
}

impl Env {
    /// Variable `name` of the login shell, when it is set to something.
    pub fn var(&self, name: &str) -> Option<&str> {
        (self.vars.get(name).map(String::as_str)).filter(|value| !value.is_empty())
    }
}

/// What an install is given beside the listing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct With {
    /// The person's answer to each need, by its name.
    pub values: BTreeMap<String, String>,
    /// A skill's folder, fetched and unpacked: what is copied.
    pub folder: Option<PathBuf>,
}

/// What an install would do, said before anything is written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Plan {
    /// Each thing written or run, in words: a file's path, a command line.
    pub steps: Vec<String>,
    /// What the person does after for the agent to see it: start it again, a command of its own.
    pub after: String,
}
