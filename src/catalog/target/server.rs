//! An MCP server put in an agent's configuration: its entry named by a short key made from the
//! listing's name (`io.github.acme/weather-server` is `weather-server`), what the person gave for
//! its needs checked first, and the agent's own part (`Servers`) asked to write it.
//!
//! An entry is known to be this listing's by what it runs, never by its name alone: the same
//! program with the same arguments (or the same address) is it; the same program whose
//! arguments differ only in a version (`pkg@1.2.0`, `pkg==1.2`) is an older one of it, and is
//! replaced; anything else of that name is the person's, and is never written over or taken
//! out. A value the person gives is written only into the agent's own entry: no plan, error or
//! log says it (a secret shows as `NAME=…`, and what a program says back has it masked).

use std::path::PathBuf;

use serde_json::Value;

use super::Agent;
use crate::catalog::capital;
use crate::catalog::{Env, Listing, Need, Plan, Recipe, Runs, With};
use crate::setup::reach::shown;

/// An agent's own part of keeping MCP servers.
pub(super) trait Servers: Sync {
    /// Whether it can reach a server that `runs` so.
    fn takes(&self, runs: &Runs) -> bool;
    /// Its file that holds the servers.
    fn file(&self, env: &Env) -> PathBuf;
    /// The entry `key` in `text` (that file), when it has one.
    fn seen(&self, text: &str, key: &str) -> Option<Seen>;
    /// `wanted` written in, by its program or into its file.
    fn add(&self, agent: &Agent, env: &Env, wanted: &Wanted<'_>) -> Result<(), String>;
    /// Entry `key` taken out.
    fn take_out(&self, agent: &Agent, env: &Env, key: &str) -> Result<(), String>;
}

/// A server as it is to be written: its key, how it runs, and the values given for it.
pub(super) struct Wanted<'a> {
    pub(super) key: String,
    pub(super) runs: &'a Runs,
    /// Its program's environment, by name.
    pub(super) env: Vec<(String, String)>,
    /// A remote one's headers, by name.
    pub(super) headers: Vec<(String, String)>,
    /// The values that are secrets: masked in whatever a program says back.
    pub(super) secrets: Vec<String>,
}

/// An entry as an agent keeps it: what it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Seen {
    pub(super) command: Option<String>,
    pub(super) args: Vec<String>,
    pub(super) url: Option<String>,
}

impl Seen {
    /// A JSON entry: a `command` (a string with `args` beside it, or a list of the program and
    /// its arguments), or a `url` (or `httpUrl`).
    pub(super) fn of(entry: &Value) -> Option<Self> {
        let strings = |value: &Value| -> Vec<String> {
            (value.as_array().into_iter().flatten())
                .filter_map(|word| word.as_str().map(String::from))
                .collect()
        };
        let (command, args) = match &entry["command"] {
            Value::String(command) => (Some(command.clone()), strings(&entry["args"])),
            line @ Value::Array(_) => {
                let mut line = strings(line);
                let command = (!line.is_empty()).then(|| line.remove(0));
                (command, line)
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::Object(_) => (None, vec![]),
        };
        let url = (entry["url"].as_str())
            .or_else(|| entry["httpUrl"].as_str())
            .map(String::from);
        (command.is_some() || url.is_some()).then_some(Self { command, args, url })
    }
}

/// What an entry of the key is to a listing.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Found {
    /// It, as it would be written.
    Same,
    /// An older version of it: the version it runs, empty when that is not plain.
    Older(String),
    /// Something else, the person's.
    Other,
}

/// What `seen` is to a listing that `runs` so.
pub(super) fn found(seen: &Seen, runs: &Runs) -> Found {
    match runs {
        Runs::Remote { url, .. } if seen.url.as_deref() == Some(url) => Found::Same,
        Runs::Remote { .. } => Found::Other,
        Runs::Command { command, args } => {
            if seen.command.as_deref() != Some(command) || seen.args.len() != args.len() {
                return Found::Other;
            }
            if seen.args == *args {
                return Found::Same;
            }
            let pairs = seen.args.iter().zip(args);
            if !pairs
                .clone()
                .all(|(had, wanted)| unversioned(had).0 == unversioned(wanted).0)
            {
                return Found::Other;
            }
            let version = (pairs.filter(|(had, wanted)| had != wanted))
                .map(|(had, _)| unversioned(had).1)
                .find(|version| !version.is_empty())
                .unwrap_or_default();
            Found::Older(version.to_string())
        }
    }
}

/// An argument and the version it pins, when it pins one as a package runner reads it:
/// `@acme/weather@1.2.0` is `@acme/weather` at `1.2.0`, `weather==1.2` is `weather` at `1.2`.
pub(super) fn unversioned(arg: &str) -> (&str, &str) {
    if let Some((name, version)) = arg.split_once("==") {
        return (name, version);
    }
    match arg.rsplit_once('@') {
        Some((name, version))
            if !name.is_empty()
                && (version == "latest" || version.starts_with(|c: char| c.is_ascii_digit())) =>
        {
            (name, version)
        }
        Some(_) | None => (arg, ""),
    }
}

/// The key a listing's server is kept by: the last part of its name, of `a-z A-Z 0-9 _ -`.
pub(super) fn key(name: &str) -> Result<String, String> {
    let last = name.rsplit('/').next().unwrap_or(name);
    let key: String = (last.chars())
        .map(
            |c| match c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                true => c,
                false => '-',
            },
        )
        .collect();
    let key: String = key.trim_matches('-').chars().take(64).collect();
    match key.is_empty() {
        true => Err(format!("`{name}` makes no name for a server")),
        false => Ok(key),
    }
}

/// The listing's server, checked: its key, how it runs, and the needs that apply to it (a
/// header for a remote one, a variable for a program).
fn shape(listing: &Listing) -> Result<(String, &Runs, Vec<&Need>), String> {
    let Recipe::Server(server) = &listing.recipe else {
        return Err("it is no MCP server".into());
    };
    match &server.runs {
        Runs::Command { command, args } => {
            let bad = |word: &str| word.contains('\0');
            if command.is_empty()
                || command.starts_with('-')
                || bad(command)
                || args.iter().any(|a| bad(a))
            {
                return Err(format!("`{command}` is no program to run"));
            }
        }
        Runs::Remote { url, .. } => {
            let parsed = url::Url::parse(url).map_err(|why| format!("`{url}`: {why}"))?;
            if !matches!(parsed.scheme(), "https" | "http") {
                return Err(format!("`{url}` is no web address"));
            }
        }
    }
    let remote = matches!(server.runs, Runs::Remote { .. });
    let needs: Vec<&Need> = (server.needs.iter())
        .filter(|need| need.header == remote)
        .collect();
    for need in &needs {
        let fits = |c: char| c.is_ascii_alphanumeric() || c == '_' || (need.header && c == '-');
        let first = need.name.chars().next();
        if !need.name.chars().all(fits) || first.is_none_or(|c| c.is_ascii_digit() || c == '-') {
            return Err(format!("`{}` is no name for a value", need.name));
        }
    }
    Ok((key(&listing.name)?, &server.runs, needs))
}

/// The server as it is to be written, with the person's values; refused when a value it needs
/// was not given (said by its name, never by a value).
pub(super) fn wanted<'a>(listing: &'a Listing, with: &With) -> Result<Wanted<'a>, String> {
    let (key, runs, needs) = shape(listing)?;
    let mut wanted = Wanted {
        key,
        runs,
        env: vec![],
        headers: vec![],
        secrets: vec![],
    };
    for need in needs {
        let given = (with.values.get(&need.name).map(String::as_str))
            .filter(|value| !value.is_empty())
            .or_else(|| Some(need.default.as_str()).filter(|value| !value.is_empty()));
        let Some(value) = given else {
            if need.required {
                return Err(format!("{} is needed first: {}", need.name, need.about));
            }
            continue;
        };
        if need.header && value.contains(['\r', '\n']) {
            return Err(format!("{} is more than one line", need.name));
        }
        if need.secret {
            wanted.secrets.push(value.to_string());
        }
        let list = match need.header {
            true => &mut wanted.headers,
            false => &mut wanted.env,
        };
        list.push((need.name.clone(), value.to_string()));
    }
    Ok(wanted)
}

/// `text` with every secret of `wanted` in it shown as `…`.
pub(super) fn masked(text: &str, wanted: &Wanted<'_>) -> String {
    (wanted.secrets.iter()).fold(text.to_string(), |text, secret| {
        text.replace(secret.as_str(), "…")
    })
}

/// What a server runs, as a person reads it: its command line, or its address.
pub(super) fn line(runs: &Runs) -> String {
    match runs {
        Runs::Remote { url, sse: true } => format!("{url} (server-sent events)"),
        Runs::Remote { url, sse: false } => url.clone(),
        Runs::Command { command, args } => (std::iter::once(command).chain(args))
            .map(|word| {
                match word.is_empty()
                    || word.contains(|c: char| c.is_whitespace() || "'\"$\\".contains(c))
                {
                    true => format!("'{}'", word.replace('\'', "'\\''")),
                    false => word.clone(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// What entry `key` of `agent`'s file is to the listing, when there is one.
fn found_in(agent: &Agent, env: &Env, key: &str, runs: &Runs) -> Option<Found> {
    let text = std::fs::read_to_string(agent.servers.file(env)).ok()?;
    Some(found(&agent.servers.seen(&text, key)?, runs))
}

/// Why an entry of the key is left as it is.
fn taken(agent: &Agent, env: &Env, key: &str) -> String {
    let at = shown(&agent.servers.file(env), &env.home);
    format!("{at} already has a server `{key}` that LLX Desk did not add: it is left as it is")
}

pub(super) fn plan(agent: &Agent, listing: &Listing, env: &Env) -> Result<Plan, String> {
    let (key, runs, needs) = shape(listing)?;
    let at = shown(&agent.servers.file(env), &env.home);
    let first = match found_in(agent, env, &key, runs) {
        None | Some(Found::Same) => format!("Add the server `{key}` to {at}: {}", line(runs)),
        Some(Found::Older(version)) if !version.is_empty() => {
            format!(
                "Replace the server `{key}` (version {version}) in {at}: {}",
                line(runs)
            )
        }
        Some(Found::Older(_)) => format!("Replace the server `{key}` in {at}: {}", line(runs)),
        Some(Found::Other) => return Err(taken(agent, env, &key)),
    };
    let mut steps = vec![first];
    for need in needs {
        let optional = if need.required { "" } else { " (if given)" };
        steps.push(match need.header {
            true => format!("With the header: {}: …{optional}", need.name),
            false => format!("With: {}=…{optional}", need.name),
        });
    }
    Ok(Plan {
        steps,
        after: capital(agent.after),
    })
}

pub(super) fn install(
    agent: &Agent,
    listing: &Listing,
    with: &With,
    env: &Env,
) -> Result<String, String> {
    let wanted = wanted(listing, with)?;
    let (key, runs) = (&wanted.key, wanted.runs);
    let done = match found_in(agent, env, key, runs) {
        Some(Found::Other) => return Err(taken(agent, env, key)),
        // Written again, so the values given now are the ones it has.
        Some(Found::Same | Found::Older(_)) => {
            (agent.servers.take_out(agent, env, key)).map_err(|why| masked(&why, &wanted))?;
            "Updated"
        }
        None => "Added",
    };
    (agent.servers.add(agent, env, &wanted)).map_err(|why| masked(&why, &wanted))?;
    Ok(format!(
        "{done} the server `{key}` in {}: {}.",
        agent.name, agent.after
    ))
}

pub(super) fn remove(agent: &Agent, listing: &Listing, env: &Env) -> Result<String, String> {
    let (key, runs, _) = shape(listing)?;
    match found_in(agent, env, &key, runs) {
        None => Ok(format!("{} has no server `{key}`.", agent.name)),
        Some(Found::Other) => Err(taken(agent, env, &key)),
        Some(Found::Same | Found::Older(_)) => {
            agent.servers.take_out(agent, env, &key)?;
            Ok(format!("Took the server `{key}` out of {}.", agent.name))
        }
    }
}

pub(super) fn has(agent: &Agent, listing: &Listing, env: &Env) -> Option<String> {
    let (key, runs, _) = shape(listing).ok()?;
    match found_in(agent, env, &key, runs)? {
        Found::Same => Some(listing.version.clone()),
        Found::Older(version) => Some(version),
        Found::Other => None,
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
