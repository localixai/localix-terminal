//! The OpenRouter key a `goal` needs: `OPENROUTER_API_KEY`, else the one `llx-desk key`
//! saved in the config directory. An agent's host often starts the MCP server without the
//! person's shell environment, so a key saved once reaches every client.

use std::io::{self, Write as _};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The key: the environment's when set, else the saved one.
pub fn find() -> Option<String> {
    choose(std::env::var("OPENROUTER_API_KEY").ok(), || {
        path().and_then(|path| read_at(&path))
    })
}

/// `$XDG_CONFIG_HOME/llx-desk/openrouter.key`, else under `~/.config`.
pub fn path() -> Option<PathBuf> {
    path_in(
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

fn path_in(config: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    config
        .filter(|dir| !dir.as_os_str().is_empty())
        .or_else(|| home.map(|home| home.join(".config")))
        .map(|dir| dir.join("llx-desk").join("openrouter.key"))
}

fn choose(env: Option<String>, saved: impl FnOnce() -> Option<String>) -> Option<String> {
    env.filter(|key| !key.trim().is_empty()).or_else(saved)
}

fn read_at(path: &Path) -> Option<String> {
    let key = std::fs::read_to_string(path).ok()?;
    let key = key.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

/// `key` saved at `path`, readable by its owner alone, in a directory only they open.
pub fn save_at(path: &Path, key: &str) -> io::Result<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the key is empty",
        ));
    }
    if let Some(dir) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    // A file there before keeps its mode on open: set it.
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    writeln!(file, "{key}")
}

#[cfg(test)]
#[path = "key_tests.rs"]
mod tests;
