//! How what runs in the app's terminals reaches the app, through their environment alone: the
//! folder of the app's command first on their `PATH`, and the variables a coding agent run
//! there finds the app's plugin or its MCP server by, each an agent's own way to add to its
//! settings without replacing the person's. Nothing is written into an agent's settings or the
//! person's home: what the app keeps for them is in its own data folder, and outside the app's
//! terminals the agents are as the person set them. An agent with no such variable finds the
//! command on the `PATH` alone.

mod claude;
mod copy;
mod layers;
mod managed;

use std::path::{Path, PathBuf};

/// The folder whose `llxterm` is this program: the bundle's `Contents/Resources/bin` (one link
/// to the program, so nothing else of the bundle is on the PATH), else the program's own folder
/// (a build).
pub fn command_dir() -> Option<PathBuf> {
    command_dir_of(crate::bundle::resources(), std::env::current_exe().ok())
}

/// [`command_dir`], for the bundle's `resources` and the program `exe`.
fn command_dir_of(resources: Option<PathBuf>, exe: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(resources) = resources {
        return Some(resources.join("bin"));
    }
    let exe = exe?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    exe.parent().map(Path::to_path_buf)
}

/// The variables a terminal is given, for an environment that `var` reads (the caller's, else
/// the app's). Read anew for each terminal: the managed settings that decide Claude Code's are
/// read as it starts, never kept from an earlier one.
pub fn variables(var: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> = claude::plugin_dirs(var).into_iter().collect();
    vars.extend(layers::layers(var, &crate::paths::data()));
    vars
}

/// `file` holding `contents`, written when it does not already: whole at once, so an agent that
/// starts meanwhile reads the old file or the new, never half. None when it cannot be.
fn written(file: &Path, contents: &str) -> Option<PathBuf> {
    if std::fs::read_to_string(file).is_ok_and(|there| there == contents) {
        return Some(file.to_path_buf());
    }
    let folder = file.parent()?;
    std::fs::create_dir_all(folder).ok()?;
    let name = file.file_name()?.to_string_lossy();
    let draft = folder.join(format!(".{name}.{}", std::process::id()));
    let done = std::fs::write(&draft, contents).and_then(|()| std::fs::rename(&draft, file));
    if let Err(why) = done {
        let _ = std::fs::remove_file(&draft);
        log::warn!("terminals go without {}: {why}", file.display());
        return None;
    }
    Some(file.to_path_buf())
}

#[cfg(test)]
#[path = "reach_tests.rs"]
mod tests;
