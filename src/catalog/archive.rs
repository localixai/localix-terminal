//! A GitHub repository at one revision, fetched as its archive and unpacked into the cache:
//! once per repository and revision, the folder reused after. The archive is opened by the
//! system's `tar` through `crate::fetch::untar`, which refuses one that reaches outside its
//! folder; a folder is in its place only once it is whole.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::source::Fetch;

/// How long unpacking may take.
const UNPACK_WITHIN: Duration = Duration::from_secs(5 * 60);

/// Whether `part` may name a folder in the cache: letters, digits, `.`, `-` and `_`, and no
/// name of a step (`.`, `..`).
fn plain(part: &str) -> bool {
    !part.is_empty()
        && part.len() <= 100
        && !part.starts_with('.')
        && (part.chars()).all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// The folder `repo` (`owner/repo`) unpacked at `rev` (a branch, a tag, a commit) is kept in.
pub fn folder(cache: &Path, repo: &str, rev: &str) -> Result<PathBuf, String> {
    let named = repo.split_once('/');
    let Some((owner, name)) = named.filter(|(owner, name)| plain(owner) && plain(name)) else {
        return Err(format!("`{repo}` is no GitHub repository"));
    };
    if !plain(rev) {
        return Err(format!("`{rev}` is no revision of {repo}"));
    }
    Ok(cache.join("archives").join(owner).join(name).join(rev))
}

/// `repo` at `rev`, unpacked: its archive fetched from GitHub and unpacked without its top
/// folder, or the folder a call before unpacked it to.
pub fn unpacked(net: &dyn Fetch, cache: &Path, repo: &str, rev: &str) -> Result<PathBuf, String> {
    let folder = folder(cache, repo, rev)?;
    if folder.is_dir() {
        return Ok(folder);
    }
    let parent = folder.parent().unwrap_or(cache);
    std::fs::create_dir_all(parent).map_err(|why| format!("{}: {why}", parent.display()))?;
    let token = crate::token();
    let archive = parent.join(format!(".{rev}-{token}.tar.gz"));
    let staged = parent.join(format!(".{rev}-{token}"));
    let url = format!("https://codeload.github.com/{repo}/tar.gz/{rev}");
    let made = net
        .file(&url, &archive)
        .and_then(|()| crate::fetch::untar(&archive, &staged, 1, UNPACK_WITHIN))
        .and_then(|()| {
            std::fs::rename(&staged, &folder).or_else(|why| match folder.is_dir() {
                // Another call put it in place first: the two are the same.
                true => Ok(()),
                false => Err(format!("{}: {why}", folder.display())),
            })
        });
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_dir_all(&staged);
    made.map(|()| folder)
        .map_err(|why| format!("{repo} at {rev} was not unpacked: {why}"))
}

/// Every revision of `repo` in the cache removed but `rev`: what a source no longer lists.
pub fn keep_only(cache: &Path, repo: &str, rev: &str) {
    let Ok(kept) = folder(cache, repo, rev) else {
        return;
    };
    let Some(Ok(entries)) = kept.parent().map(std::fs::read_dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        // A folder being unpacked (its name starts with a dot) is another call's.
        let staged = entry.file_name().to_string_lossy().starts_with('.');
        if path != kept && !staged && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

#[cfg(test)]
#[path = "archive_tests.rs"]
pub(crate) mod tests;
