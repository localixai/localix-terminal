//! A project's files as the window's tree shows them. A folder is read one at a time, and only
//! one the person opened: its entries in the order a person expects (folders first, then by name
//! as a file manager sorts them). `watch` hears a folder change from the system as it happens,
//! `kept` is the folders a person left open in each project, `ops` is what they do to an entry
//! from the tree, and `text` is one file as an editor holds it.

pub mod drafts;
mod kept;
pub mod ops;
mod order;
pub mod text;
mod watch;

use std::io;
use std::path::{Component, Path, PathBuf};

pub use kept::{moved, open, open_dirs, shut};
pub use watch::{Change, Watch};

/// The most entries of one folder the tree lists; what is past them is counted, not shown.
pub const MOST: usize = 2000;

/// Names no tree shows: the repository's own store, and what the Finder leaves in a folder.
const UNSHOWN: [&str; 2] = [".git", ".DS_Store"];

/// One entry of a folder: its name, and whether it is a folder (a link to one is one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
}

/// A folder as it was read: its entries in order, and how many more it holds past [`MOST`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Listing {
    pub entries: Vec<Entry>,
    pub more: usize,
}

/// The entries of `dir`, read once: no file is opened and no folder below is looked into.
pub fn list(dir: &Path) -> io::Result<Listing> {
    let mut entries = vec![];
    for entry in std::fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        if UNSHOWN.contains(&name.as_str()) {
            continue;
        }
        // The entry's own kind costs nothing more; only a link is followed, to say what it is.
        let dir = match entry.file_type() {
            Ok(kind) if kind.is_symlink() => entry.path().is_dir(),
            Ok(kind) => kind.is_dir(),
            Err(_) => false,
        };
        entries.push(Entry { name, dir });
    }
    entries.sort_unstable_by(|a, b| {
        (b.dir.cmp(&a.dir)).then_with(|| order::natural(&a.name, &b.name))
    });
    let more = entries.len().saturating_sub(MOST);
    entries.truncate(MOST);
    Ok(Listing { entries, more })
}

/// The folder `rel` names under `root` (`""`: the root itself), as the tree writes a path:
/// names joined by `/`, none of them `.` or `..`. Anything else names nothing.
pub fn within(root: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() {
        return Some(root.to_path_buf());
    }
    let plain = rel.split('/').all(|name| {
        !name.is_empty()
            && !name.contains('\0')
            && matches!(
                Path::new(name).components().next(),
                Some(Component::Normal(_))
            )
    });
    plain.then(|| root.join(rel))
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
