//! Claude Code in the app's terminals: its plugin loaded from the app's copy through
//! `CLAUDE_CODE_PLUGIN_DIRS`, nothing installed and nothing written into its settings. Claude
//! Code reads the variable from 2.1.280 (the plugin's hooks module runs from 2.1.287); an older
//! one goes without the plugin. Where its managed settings refuse plugin folders (`claude`
//! would not start with one named), the variable names none of the app's.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::{copy, managed};

const VARIABLE: &str = "CLAUDE_CODE_PLUGIN_DIRS";

/// The variable for a terminal whose environment `var` reads: the person's folders and the
/// app's copy of the plugin. None when there is nothing to change.
pub(super) fn plugin_dirs(var: &dyn Fn(&str) -> Option<String>) -> Option<(String, String)> {
    let takes = managed::takes_plugin_folders(&managed::Sources::here(var));
    let theirs = var(VARIABLE).filter(|dirs| !dirs.is_empty());
    // Refused, a copy is not even made: only an inherited mention of it is taken out.
    let ours = if takes { copied()? } else { copy_place()? };
    let value = joined(theirs, ours.to_str()?, takes)?;
    Some((VARIABLE.into(), value))
}

/// `theirs` (the folders the person names) with `ours` after them when Claude Code `takes`
/// plugin folders, else without it; none when that is what is there already, or when `ours`
/// cannot be named in the list.
pub(super) fn joined(theirs: Option<String>, ours: &str, takes: bool) -> Option<String> {
    if ours.contains(':') {
        return None;
    }
    let named = |dirs: &str| dirs.split(':').any(|dir| dir == ours);
    match (theirs, takes) {
        (None, true) => Some(ours.to_string()),
        (Some(theirs), true) if named(&theirs) => Some(theirs),
        (Some(theirs), true) => Some(format!("{theirs}:{ours}")),
        (Some(theirs), false) if named(&theirs) => {
            let kept: Vec<&str> = theirs.split(':').filter(|dir| *dir != ours).collect();
            Some(kept.join(":"))
        }
        (_, false) => None,
    }
}

/// The plugin as the app has it, where its copy is kept, and whether to copy it again each run
/// (a source tree's, which changes under one version): the bundle's, else this build's source
/// tree's when it is there.
fn source() -> Option<(PathBuf, PathBuf, bool)> {
    let data = crate::paths::data();
    if let Some(resources) = crate::bundle::resources() {
        let plugin = resources.join("claude-code");
        return Some((plugin, data.join("claude-code"), false));
    }
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins/claude-code");
    (tree.join(".claude-plugin/plugin.json").is_file())
        .then(|| (tree, data.join("claude-code-tree"), true))
}

/// Where this version's copy is, made or not.
fn copy_place() -> Option<PathBuf> {
    let (_, under, _) = source()?;
    Some(under.join(env!("CARGO_PKG_VERSION")))
}

/// This version's copy, made once a run, as the first terminal starts.
fn copied() -> Option<PathBuf> {
    static COPY: OnceLock<Option<PathBuf>> = OnceLock::new();
    let made = COPY.get_or_init(|| {
        let (plugin, under, again) = source()?;
        copy::kept(&plugin, &under, env!("CARGO_PKG_VERSION"), again)
            .inspect_err(|why| log::warn!("terminals go without the Claude Code plugin: {why}"))
            .ok()
    });
    made.clone()
}
