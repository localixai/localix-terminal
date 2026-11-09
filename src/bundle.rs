//! Where this program lives as an app: inside `LLX Desk.app` (macOS, scripts/bundle-macos.sh)
//! it is the app itself, with its name, its icon and its Dock place, and LaunchServices starts
//! it; anywhere else (a build, a package on Linux) it is a plain program, started as one. The
//! layers ask here, never guess from paths of their own.

use std::path::{Path, PathBuf};

/// The app bundle this program is in (`…/LLX Desk.app`), its path resolved through links (the
/// `llx-desk` command is a link into the bundle); none for a program outside one.
pub fn app() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    of(&exe).map(Path::to_path_buf)
}

/// Whether this app is where it stays: not run from a mounted disk image or from the place the
/// system moves a downloaded one to (App Translocation), both gone at eject or restart. What is
/// written into the person's tools names this program, so it waits for a bundle that is in
/// place. A program outside a bundle is always in place.
pub fn in_place() -> bool {
    app().is_none_or(|app| !temporary(&app))
}

/// Whether `app` is in a place that does not last.
fn temporary(app: &Path) -> bool {
    app.starts_with("/Volumes")
        || app
            .components()
            .any(|part| part.as_os_str() == "AppTranslocation")
}

/// Whether this run is the app opened (the Finder, the Dock, LaunchServices): the program run
/// by its own path in the bundle, not through the `llx-desk` link, given nothing but what the
/// system adds (`-psn_…` from older macOS, `-ApplePersistenceIgnoreState YES` when it comes
/// back). Run through the link with nothing, it is the command, and says how to use it.
pub fn opened(args: &[String]) -> bool {
    let bundled = std::env::current_exe().is_ok_and(|exe| of(&exe).is_some());
    bundled && adds_only_options(args)
}

/// Whether `args` are what the system adds: none, or options (no command starts with `-`),
/// but the ones that ask for help or the version.
fn adds_only_options(args: &[String]) -> bool {
    args.first().is_none_or(|first| {
        first.starts_with('-') && !["-h", "--help", "-V", "--version"].contains(&first.as_str())
    })
}

/// The bundle `exe` is the program of: `<x>.app/Contents/MacOS/<exe>`.
fn of(exe: &Path) -> Option<&Path> {
    let mut up = exe.ancestors().skip(1);
    let (macos, contents, app) = (up.next()?, up.next()?, up.next()?);
    let named = |dir: &Path, name: &str| dir.file_name().is_some_and(|file| file == name);
    (named(macos, "MacOS")
        && named(contents, "Contents")
        && app.extension().is_some_and(|ext| ext == "app"))
    .then_some(app)
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
