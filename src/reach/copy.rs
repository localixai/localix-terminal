//! The plugin's copy in the app's own folder. Claude Code writes into a plugin folder it loads
//! (the declarations it gives a hooks module), so it is never handed the signed bundle: it is
//! handed a copy, made once per version, whole or not at all.

use std::io;
use std::path::{Path, PathBuf};

/// What a copy leaves out wherever it is (what the bundle leaves out), and the folder Claude
/// Code writes itself.
const LEFT_OUT: [&str; 2] = ["node_modules", ".DS_Store"];
const WRITTEN: &str = ".claude-plugin/types";

/// The copy of the plugin at `source` as `under/name`, made when it is not there (a folder of
/// that name with the plugin's manifest) or when `again`. It is made beside as a draft and put
/// in place by a rename, so a copy cut short is never used; once it is in place, the copies of
/// other names under `under` go.
pub(super) fn kept(source: &Path, under: &Path, name: &str, again: bool) -> io::Result<PathBuf> {
    let copy = under.join(name);
    if !again && copy.join(".claude-plugin").join("plugin.json").is_file() {
        return Ok(copy);
    }
    std::fs::create_dir_all(under)?;
    let pid = std::process::id();
    let draft = under.join(format!(".{name}.{pid}"));
    let _ = std::fs::remove_dir_all(&draft);
    if let Err(why) = copied(source, &draft, source) {
        let _ = std::fs::remove_dir_all(&draft);
        return Err(why);
    }
    let old = under.join(format!(".{name}.old.{pid}"));
    if copy.symlink_metadata().is_ok() {
        std::fs::rename(&copy, &old)?;
    }
    std::fs::rename(&draft, &copy)?;
    let _ = std::fs::remove_dir_all(&old);
    for entry in std::fs::read_dir(under)?.filter_map(Result::ok) {
        let other = entry.file_name();
        if other != name && !other.to_string_lossy().starts_with('.') {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    Ok(copy)
}

/// The folder `from` (of the plugin at `root`) copied to `to`: its files with their modes, its
/// links as links, without what is left out.
fn copied(from: &Path, to: &Path, root: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let within = path.strip_prefix(root).unwrap_or(&path);
        if LEFT_OUT.iter().any(|left| entry.file_name() == *left) || within == Path::new(WRITTEN) {
            continue;
        }
        let target = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&path)?, &target)?;
        } else if kind.is_dir() {
            copied(&path, &target, root)?;
        } else {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "copy_tests.rs"]
mod tests;
