//! A link of the document a file's tab draws, followed to another file: read from the folder of
//! the file that holds it (from the project's own where it starts with `/`), it opens as a file
//! picked in the tree does, and only where it is the project's.

use std::path::{Path, PathBuf};

use super::Desk;
use super::edit_disk::within;
use crate::browser::Asked;

impl Desk {
    /// The person followed `href` in the document of file `key`'s tab: the file it names opens
    /// in a tab of theirs. What names no file of the project opens nothing.
    pub(super) fn file_followed(&self, key: &str, href: &str) {
        let held =
            (crate::lock(&self.edits).get(key)).map(|kept| (kept.project, kept.path.clone()));
        let Some((project, from)) = held else {
            return;
        };
        let Some(root) = self.project_by_id(project).map(|known| known.path) else {
            return;
        };
        let Some(path) = linked(&from, &root, href).and_then(|path| path.canonicalize().ok())
        else {
            log::debug!("the link {href} of {} names no file", from.display());
            return;
        };
        if let Err(not) = within(&path, &root) {
            log::debug!("a link was not followed: {}", not.why);
            return;
        }
        if let Some(desk) = self.me.upgrade() {
            desk.person_asked(&crate::projects::key(project), Asked::File(path));
        }
    }
}

/// The file `href` names in the document at `from`, a file of the project at `root`: its path
/// without what follows `#` or `?`, its `%` escapes read. None for an address with a scheme or
/// a host, and for one that names only a place in the document itself.
pub(super) fn linked(from: &Path, root: &Path, href: &str) -> Option<PathBuf> {
    let path = href.split(['#', '?']).next().unwrap_or_default();
    let scheme = (path.split_once(':'))
        .is_some_and(|(before, _)| !before.is_empty() && !before.contains('/'));
    if path.is_empty() || scheme || path.starts_with("//") {
        return None;
    }
    let path = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .ok()?;
    Some(match path.strip_prefix('/') {
        Some(inside) => root.join(inside),
        None => from.parent()?.join(&*path),
    })
}

#[cfg(test)]
#[path = "edit_link_tests.rs"]
mod tests;
