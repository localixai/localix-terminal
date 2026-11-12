//! A plugin put in by its agent's own program, never by writing its files: the marketplace
//! added first when the agent does not know it (`<agent> plugin marketplace add <owner/repo>`),
//! then `name@marketplace` installed. What the agent records of it is read from its files alone
//! (`Plugins::has`). Nothing is passed that would accept a command for the person (`-y`), nor
//! that would trust a plugin's hooks for them.

use std::time::Duration;

use serde_json::Value;

use super::Agent;
use crate::catalog::capital;
use crate::catalog::{Env, Listing, Plan, PluginFormat, Recipe};
use crate::setup::reach::{Ran, failure};

/// How long a plugin's command may take: a marketplace is cloned, a plugin copied.
const MOST: Duration = Duration::from_secs(180);

/// A plugin as its agent's program names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Plugin<'a> {
    pub(super) name: &'a str,
    pub(super) marketplace: &'a str,
    /// Where its marketplace is added from: `owner/repo`.
    pub(super) from: &'a str,
}

impl Plugin<'_> {
    /// `name@marketplace`.
    pub(super) fn id(&self) -> String {
        format!("{}@{}", self.name, self.marketplace)
    }
}

/// An agent's own part of installing plugins.
pub(super) trait Plugins: Sync {
    /// Whether its program reads marketplaces of `format`.
    fn takes(&self, format: PluginFormat) -> bool;
    /// Whether it knows marketplace `name` already, from its records.
    fn known(&self, env: &Env, name: &str) -> bool;
    /// Its program's words that add the marketplace at `from`.
    fn add_market(&self, from: &str) -> Vec<String>;
    /// Its words that install plugin `id`, and the ones that uninstall it.
    fn install(&self, id: &str) -> Vec<String>;
    fn uninstall(&self, id: &str) -> Vec<String>;
    /// Whether what its program said is that the act was done (or had been already).
    fn outcome(&self, ran: &Ran) -> Result<(), String>;
    /// The version of `plugin` its records have, empty when they name none.
    fn has(&self, env: &Env, plugin: &Plugin<'_>) -> Option<String>;
    /// What to do after, for it to load the plugin.
    fn after(&self) -> &'static str;
}

/// Whether `word` can be passed to a program as a name: no flag, no path, no space.
fn plain(word: &str) -> bool {
    !word.is_empty()
        && !word.starts_with(['-', '.'])
        && (word.chars()).all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// The listing's plugin, checked: its name and marketplace plain words, its source `owner/repo`.
pub(super) fn plugin(listing: &Listing) -> Result<Plugin<'_>, String> {
    let Recipe::Plugin {
        marketplace, from, ..
    } = &listing.recipe
    else {
        return Err("it is no plugin".into());
    };
    let repo = from
        .split_once('/')
        .is_some_and(|(owner, repo)| plain(owner) && plain(repo));
    match (plain(&listing.name), plain(marketplace), repo) {
        (true, true, true) => Ok(Plugin {
            name: &listing.name,
            marketplace,
            from,
        }),
        (false, _, _) => Err(format!("`{}` is no plugin's name", listing.name)),
        (_, false, _) => Err(format!("`{marketplace}` is no marketplace's name")),
        (_, _, false) => Err(format!("`{from}` is no GitHub repository (owner/repo)")),
    }
}

/// A command line as the person reads it: without the words that only shape its answer.
fn shown(agent: &Agent, words: &[String]) -> String {
    let words: Vec<&str> = (words.iter())
        .map(String::as_str)
        .filter(|word| *word != "--json")
        .collect();
    format!("Run: {} {}", agent.program, words.join(" "))
}

fn plugins(agent: &Agent) -> Result<&'static dyn Plugins, String> {
    agent
        .plugins
        .ok_or_else(|| format!("{} takes no plugins", agent.name))
}

pub(super) fn plan(agent: &Agent, listing: &Listing, env: &Env) -> Result<Plan, String> {
    let (it, plugin) = (plugins(agent)?, plugin(listing)?);
    let mut steps = vec![];
    if !it.known(env, plugin.marketplace) {
        steps.push(shown(agent, &it.add_market(plugin.from)));
    }
    steps.push(shown(agent, &it.install(&plugin.id())));
    Ok(Plan {
        steps,
        after: capital(it.after()),
    })
}

/// `words` run by `agent`'s program, and what it said judged.
fn act(agent: &Agent, env: &Env, it: &dyn Plugins, words: &[String]) -> Result<(), String> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    it.outcome(&agent.run(env, &words, MOST)?)
}

pub(super) fn install(agent: &Agent, listing: &Listing, env: &Env) -> Result<String, String> {
    let (it, plugin) = (plugins(agent)?, plugin(listing)?);
    // A marketplace the records do not list may still be known (they lag, or were moved): the
    // install says whether it is, and its failure is told only when the install fails too.
    let market = match it.known(env, plugin.marketplace) {
        true => Ok(()),
        false => act(agent, env, it, &it.add_market(plugin.from)),
    };
    if let Err(why) = act(agent, env, it, &it.install(&plugin.id())) {
        return Err(match market {
            Ok(()) => why,
            Err(first) => format!("{first}; {why}"),
        });
    }
    Ok(format!(
        "Installed {} in {}: {}.",
        plugin.id(),
        agent.name,
        it.after()
    ))
}

pub(super) fn remove(agent: &Agent, listing: &Listing, env: &Env) -> Result<String, String> {
    let (it, plugin) = (plugins(agent)?, plugin(listing)?);
    act(agent, env, it, &it.uninstall(&plugin.id()))?;
    Ok(format!("Uninstalled {} from {}.", plugin.id(), agent.name))
}

pub(super) fn has(agent: &Agent, listing: &Listing, env: &Env) -> Option<String> {
    agent.plugins?.has(env, &plugin(listing).ok()?)
}

/// `words` as a program's arguments.
pub(super) fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| word.to_string()).collect()
}

/// What a program that answers with its exit status alone said: done when it ended well.
pub(super) fn by_status(ran: &Ran) -> Result<(), String> {
    match ran.ok {
        true => Ok(()),
        false => Err(failure(ran, "it failed and said nothing")),
    }
}

/// What Claude Code's `--json` said: its last line that is a JSON object with an `outcome`.
/// Done when it is `ok`, or when the plugin already was as asked (installed, or not there to
/// uninstall); else its message.
pub(super) fn claude_outcome(ran: &Ran) -> Result<(), String> {
    let result: Option<Value> = (ran.out.lines().rev())
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .find(|line| line.get("outcome").is_some());
    let Some(result) = result else {
        return by_status(ran);
    };
    let already = result["alreadyInGoalState"] == true
        || matches!(
            result["failureCode"].as_str(),
            Some("already_in_goal_state" | "not_installed")
        );
    match result["outcome"].as_str() {
        Some("ok") => Ok(()),
        _ if already => Ok(()),
        _ => Err((result["message"].as_str())
            .map_or_else(|| failure(ran, "it failed and said nothing"), String::from)),
    }
}

#[cfg(test)]
#[path = "plugin_tests.rs"]
mod tests;
