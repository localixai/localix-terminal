//! The params of a `window/call`, as every client builds them.

use std::path::PathBuf;

use serde_json::{Value, json};

/// Who calls: the project it works in (a folder, absolute) and its name within it, if it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub project: PathBuf,
    pub agent: Option<String>,
}

impl Caller {
    /// The caller in `cwd`, named `agent`: its project is the one [`project_of`] says, from
    /// `LLXTERM_PROJECT` and the terminal's `LLXTERM_PANE_PROJECT`.
    pub fn of(cwd: &std::path::Path, agent: Option<String>) -> Self {
        let named = std::env::var_os("LLXTERM_PROJECT");
        let project = project_of(cwd, named, std::env::var_os("LLXTERM_PANE_PROJECT"));
        let project = std::path::absolute(&project).unwrap_or(project);
        Self { project, agent }
    }

    /// The caller in project `project` (the project around it, if it is a folder within one),
    /// named `agent`: an integration that knows its project says so.
    pub fn at(project: &std::path::Path, agent: Option<String>) -> Self {
        let project = crate::projects::root_of(project);
        let project = std::path::absolute(&project).unwrap_or(project);
        Self { project, agent }
    }

    /// The caller in this process's directory.
    pub fn here(agent: Option<String>) -> Self {
        Self::of(&std::env::current_dir().unwrap_or_default(), agent)
    }

    /// Its params, as every method that is a caller's takes them: `project`, and `agent`.
    pub fn params(&self) -> serde_json::Map<String, Value> {
        let mut params = serde_json::Map::new();
        params.insert("project".into(), json!(self.project.display().to_string()));
        if let Some(agent) = &self.agent {
            params.insert("agent".into(), json!(agent));
        }
        params
    }

    /// Its params and `more`.
    pub fn with(&self, more: Value) -> Value {
        let mut params = self.params();
        if let Value::Object(more) = more {
            params.extend(more);
        }
        Value::Object(params)
    }
}

/// The project of a caller in `cwd`: `named` when set (an integration that knows it better),
/// else the repository around `cwd`; in no repository, the project of the terminal of the app's
/// it runs in (`pane`) when `cwd` is in that project's folder (a folder project, or a call from
/// tmux whose lineage is lost), else `cwd`.
fn project_of(
    cwd: &std::path::Path,
    named: Option<std::ffi::OsString>,
    pane: Option<std::ffi::OsString>,
) -> PathBuf {
    let set =
        |dir: Option<std::ffi::OsString>| dir.filter(|dir| !dir.is_empty()).map(PathBuf::from);
    if let Some(named) = set(named) {
        return named;
    }
    if let Some(repository) = crate::projects::repository_of(cwd) {
        return repository;
    }
    set(pane)
        .filter(|pane| cwd.starts_with(pane))
        .unwrap_or_else(|| cwd.to_path_buf())
}

/// The variables a program the desk starts gets: this process's, but those the caller's
/// integration names its own in `LLXTERM_SCRUB` (comma-separated names; `NAME_*` names all that
/// begin so), which say what session called, not what the person set.
pub(super) fn scrubbed() -> Vec<(String, String)> {
    let scrub = std::env::var("LLXTERM_SCRUB").unwrap_or_default();
    scrubbed_of(std::env::vars(), &scrub)
}

fn scrubbed_of(vars: impl Iterator<Item = (String, String)>, scrub: &str) -> Vec<(String, String)> {
    let named = |name: &str| {
        name == "LLXTERM_SCRUB"
            || scrub
                .split(',')
                .map(str::trim)
                .any(|pattern| match pattern.strip_suffix('*') {
                    Some(prefix) => !prefix.is_empty() && name.starts_with(prefix),
                    None => pattern == name,
                })
    };
    vars.filter(|(name, _)| !named(name)).collect()
}

/// The environment a call that may start a program sends: this process's, scrubbed.
pub fn environment() -> serde_json::Map<String, Value> {
    (scrubbed().into_iter())
        .map(|(name, value)| (name, json!(value)))
        .collect()
}

/// A `window/call` request's params. The environment goes with a call that opens a tab (the
/// project's terminals, the agent's or the person's, start with it) or names a program to run.
pub fn window_params(caller: &Caller, cwd: &str, args: &Value) -> Value {
    let starts =
        args.get("action").and_then(Value::as_str) == Some("open") || args.get("command").is_some();
    let env = starts.then(environment);
    caller.with(json!({
        "cwd": cwd,
        "env": env,
        "args": args,
        "idempotencyKey": crate::token(),
    }))
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
