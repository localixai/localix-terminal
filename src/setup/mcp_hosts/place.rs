//! Where an agent keeps its configuration: under the home directory, unless the person moved
//! it with the agent's own variable (`CODEX_HOME`), or the agent keeps one per profile and the
//! person switched to another. The file the agent itself reads is the one looked at and the
//! one its `mcp add` writes, so the two never disagree.

use std::path::{Path, PathBuf};

/// Where an agent's configuration is.
#[derive(Debug, Clone, Copy)]
pub(super) struct Place {
    /// The folder it is in, under the home directory.
    pub(super) dir: &'static str,
    /// The variable that names another folder in place of that one.
    pub(super) var: &'static str,
    /// The file, under the folder.
    pub(super) file: &'static str,
    /// Whether the folder holds profiles, each a folder of its own under `profiles`, the one in
    /// use named in `active_profile`.
    pub(super) profiles: bool,
}

impl Place {
    /// The configuration file, for the person whose home is `home` and whose variables `var`
    /// reads.
    pub(super) fn config(&self, home: &Path, var: impl Fn(&str) -> Option<String>) -> PathBuf {
        let moved = var(self.var).filter(|dir| !dir.trim().is_empty());
        let root = moved.map_or_else(|| home.join(self.dir), PathBuf::from);
        match self.profiles.then(|| profile(&root)).flatten() {
            Some(name) => root.join("profiles").join(name).join(self.file),
            None => root.join(self.file),
        }
    }
}

/// The profile in use under `root`, when it is not the default one: none where `root` is a
/// profile's own folder already, and none for a name that is not one folder's.
fn profile(root: &Path) -> Option<String> {
    let in_profile = (root.parent().and_then(Path::file_name)).is_some_and(|dir| dir == "profiles");
    if in_profile {
        return None;
    }
    let named = std::fs::read_to_string(root.join("active_profile")).ok()?;
    let name = named.trim_start_matches('\u{feff}').trim();
    let one_folder = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
    (one_folder && name != "default").then(|| name.to_string())
}

/// `file` as the person reads it: under their home, from `~`.
pub(in crate::setup) fn shown(file: &Path, home: &Path) -> String {
    match file.strip_prefix(home) {
        Ok(under) => format!("~/{}", under.display()),
        Err(_) => file.display().to_string(),
    }
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
