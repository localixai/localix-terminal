//! Where a listing is installed: the port an agent is written through (`Target`), and the
//! person's coding agents behind it, each a row of what it reads and how (`Agent`, one file
//! each: claude.rs, codex.rs, gemini.rs, copilot.rs, opencode.rs).
//!
//! A kind of listing is put in the same way for every agent, and only what differs is the
//! agent's: a skill is a folder copied where it reads skills (skill.rs), an MCP server an entry
//! in its configuration (server.rs), a plugin its own program's install (plugin.rs). Where an
//! agent keeps things comes from `setup`, which knows it for the app's own entries. Everything
//! is under the `Env` given (its home, its variables), and a program is looked for on its PATH
//! only, so a test names a temporary home and never the person's. What is installed is read
//! from the agents' files alone (`has`): no program starts to draw the Store.

mod claude;
mod codex;
mod copilot;
mod gemini;
mod opencode;
mod plugin;
mod server;
mod skill;

use std::path::PathBuf;
use std::time::Duration;

use super::{Env, Listing, Plan, Recipe, With};
use crate::setup::reach::{self, Ran};
use plugin::Plugins;
use server::Servers;

/// A coding agent the Store puts listings into.
pub trait Target: Sync {
    /// The agent's id, as the window's agents (browser/agents.rs) and setup name it: `claude`.
    fn id(&self) -> &'static str;
    /// Its name where a person reads it: `Claude Code`.
    fn name(&self) -> &'static str;
    /// Whether its program is on the PATH of `env` (nothing is started).
    fn present(&self, env: &Env) -> bool;
    /// Whether it can have `listing` at all.
    fn takes(&self, listing: &Listing) -> bool;
    /// What an install would write or run, in words, before anything is done.
    fn plan(&self, listing: &Listing, env: &Env) -> Result<Plan, String>;
    /// `listing` put in; what was done, in one sentence, with what to do after.
    fn install(&self, listing: &Listing, with: &With, env: &Env) -> Result<String, String>;
    /// `listing` taken out; what was done, in one sentence.
    fn remove(&self, listing: &Listing, env: &Env) -> Result<String, String>;
    /// The version of `listing` it has (empty when that is not known), none when it has none.
    /// It reads files only.
    fn has(&self, listing: &Listing, env: &Env) -> Option<String>;
}

/// One coding agent: its program, where it reads skills, how it keeps MCP servers, and the
/// plugins its program installs.
struct Agent {
    id: &'static str,
    name: &'static str,
    program: &'static str,
    /// The folder it reads the person's own skills from.
    skills: fn(&Env) -> PathBuf,
    /// The other agents that read that same folder, in words, when it is shared.
    shared: &'static str,
    /// What to do after a skill or a server is put in, for it to see it.
    after: &'static str,
    servers: &'static dyn Servers,
    plugins: Option<&'static dyn Plugins>,
}

static ALL: [&dyn Target; 5] = [
    &claude::CLAUDE,
    &codex::CODEX,
    &gemini::GEMINI,
    &copilot::COPILOT,
    &opencode::OPENCODE,
];

/// Every agent the Store puts listings into, in the window's order (agents.rs).
pub fn all() -> &'static [&'static dyn Target] {
    &ALL
}

/// The person's machine as their agents see it: their home, and the PATH and the agents'
/// variables of their login shell. Asking the shell may take seconds: never on the window's
/// thread; one `Env` serves every call after.
pub fn env() -> Env {
    Env {
        home: std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default(),
        path: crate::setup::login_path().unwrap_or_default(),
        vars: reach::login_vars(),
    }
}

impl Target for Agent {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn present(&self, env: &Env) -> bool {
        reach::which(self.program, &env.path).is_some()
    }

    fn takes(&self, listing: &Listing) -> bool {
        match &listing.recipe {
            Recipe::Skill { .. } => true,
            Recipe::Server(server) => self.servers.takes(&server.runs),
            Recipe::Plugin { format, .. } => self.plugins.is_some_and(|it| it.takes(*format)),
        }
    }

    fn plan(&self, listing: &Listing, env: &Env) -> Result<Plan, String> {
        self.taken(listing)?;
        match &listing.recipe {
            Recipe::Skill { .. } => skill::plan(self, listing, env),
            Recipe::Server(_) => server::plan(self, listing, env),
            Recipe::Plugin { .. } => plugin::plan(self, listing, env),
        }
    }

    fn install(&self, listing: &Listing, with: &With, env: &Env) -> Result<String, String> {
        self.taken(listing)?;
        match &listing.recipe {
            Recipe::Skill { .. } => skill::install(self, listing, with, env),
            Recipe::Server(_) => server::install(self, listing, with, env),
            Recipe::Plugin { .. } => plugin::install(self, listing, env),
        }
    }

    fn remove(&self, listing: &Listing, env: &Env) -> Result<String, String> {
        self.taken(listing)?;
        match &listing.recipe {
            Recipe::Skill { .. } => skill::remove(self, listing, env),
            Recipe::Server(_) => server::remove(self, listing, env),
            Recipe::Plugin { .. } => plugin::remove(self, listing, env),
        }
    }

    fn has(&self, listing: &Listing, env: &Env) -> Option<String> {
        if !self.takes(listing) {
            return None;
        }
        match &listing.recipe {
            Recipe::Skill { .. } => skill::has(self, listing, env),
            Recipe::Server(_) => server::has(self, listing, env),
            Recipe::Plugin { .. } => plugin::has(self, listing, env),
        }
    }
}

impl Agent {
    /// Refused, in words, when it cannot have `listing`.
    fn taken(&self, listing: &Listing) -> Result<(), String> {
        match self.takes(listing) {
            true => Ok(()),
            false => Err(format!(
                "{} cannot have this {}",
                self.name,
                listing.what.name()
            )),
        }
    }

    /// Its program run with `args` as the person's terminal would, within `most`.
    fn run(&self, env: &Env, args: &[&str], most: Duration) -> Result<Ran, String> {
        let program = reach::which(self.program, &env.path).ok_or_else(|| {
            format!(
                "{} is not installed (no `{}` on the PATH)",
                self.name, self.program
            )
        })?;
        let mut vars = env.vars.clone();
        if !env.home.as_os_str().is_empty() {
            vars.insert("HOME".into(), env.home.display().to_string());
        }
        reach::run(&program, args, &env.path, &vars, most)
    }
}

/// Variable `name` of `env`, as setup's places read one.
fn var(env: &Env) -> impl Fn(&str) -> Option<String> + '_ {
    |name| env.var(name).map(String::from)
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
