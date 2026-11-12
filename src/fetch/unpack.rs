//! An archive opened by the system's tools, never trusted to stay in its folder by itself: its
//! names are read first, and one that starts at the root or steps up refuses it whole; once
//! unpacked, a link in it that points out of the folder refuses it too.

use std::path::{Component, Path};
use std::process::Command;
use std::time::Duration;

use super::run;

/// The names of what `archive` (a zip, else a tarball) holds, a line each, read by way of a
/// file in `dir`.
pub fn listed(archive: &Path, dir: &Path, most: Duration) -> Result<String, String> {
    let list = dir.join(format!("listing-{}", crate::token()));
    let failed = |why: std::io::Error| format!("{}: {why}", list.display());
    let out = std::fs::File::create(&list).map_err(failed)?;
    let zip = archive.extension().is_some_and(|kind| kind == "zip");
    let mut command = Command::new(if zip { "/usr/bin/zipinfo" } else { "tar" });
    command.arg(if zip { "-1" } else { "-tzf" }).arg(archive);
    let ran = run::within_into(command, out.into(), most, &|| {});
    let names = std::fs::read_to_string(&list).map_err(failed);
    let _ = std::fs::remove_file(&list);
    ran?;
    names
}

/// Whether every one of `names` (an archive's, a line each) unpacks inside the folder it is
/// unpacked in: none from the root, none a step up.
pub fn inside(names: &str) -> Result<(), String> {
    let out = |name: &&str| name.starts_with('/') || name.split('/').any(|part| part == "..");
    match names.lines().find(out) {
        Some(name) => Err(format!("the archive holds `{name}`, outside its folder")),
        None => Ok(()),
    }
}

/// The tarball `archive` unpacked into `into`, made anew, its first `strip` folders left out
/// (a repository's archive holds one folder of its name). Owners are not taken from it. What
/// would reach outside `into` refuses it, and `into` is then removed.
pub fn untar(archive: &Path, into: &Path, strip: u32, most: Duration) -> Result<(), String> {
    let made = |why: std::io::Error| format!("{}: {why}", into.display());
    let _ = std::fs::remove_dir_all(into);
    std::fs::create_dir_all(into).map_err(made)?;
    let unpacked = listed(archive, into, most)
        .and_then(|names| inside(&names))
        .and_then(|()| {
            let mut tar = Command::new("tar");
            tar.arg("-xzf").arg(archive).arg("-C").arg(into).args([
                "--no-same-owner",
                "--strip-components",
                &strip.to_string(),
            ]);
            run::within(tar, most, &|| {})
        })
        .and_then(|()| links_inside(into, into));
    if unpacked.is_err() {
        let _ = std::fs::remove_dir_all(into);
    }
    unpacked
}

/// Whether every link under `dir` points inside `root`, read by its words alone: a link is
/// never followed to learn where it goes.
fn links_inside(root: &Path, dir: &Path) -> Result<(), String> {
    let failed = |why: std::io::Error| format!("{}: {why}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(failed)? {
        let path = entry.map_err(failed)?.path();
        let kind = std::fs::symlink_metadata(&path)
            .map_err(failed)?
            .file_type();
        if kind.is_symlink() {
            let to = std::fs::read_link(&path).map_err(failed)?;
            let from = path
                .parent()
                .and_then(|parent| parent.strip_prefix(root).ok());
            if !from.is_some_and(|from| stays_inside(&from.join(&to))) {
                let name = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                return Err(format!(
                    "the archive's link `{name}` points outside its folder"
                ));
            }
        } else if kind.is_dir() {
            links_inside(root, &path)?;
        }
    }
    Ok(())
}

/// Whether `path`, relative to a folder, names something inside it: it never starts at the
/// root, and never steps up past where it began.
fn stays_inside(path: &Path) -> bool {
    let mut depth = 0usize;
    for part in path.components() {
        match part {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => match depth.checked_sub(1) {
                Some(up) => depth = up,
                None => return false,
            },
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

#[cfg(test)]
#[path = "unpack_tests.rs"]
mod tests;
