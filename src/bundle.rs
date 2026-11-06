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

/// Whether this run is the app opened (the Finder, the Dock, LaunchServices): the program run
/// by its own path in the bundle, not through the `llx-desk` link, given nothing but what the
/// system adds (`-psn_…`, from older macOS). Run through the link with nothing, it is the
/// command, and says how to use it.
pub fn opened(args: &[String]) -> bool {
    let bundled = std::env::current_exe().is_ok_and(|exe| of(&exe).is_some());
    bundled && args.iter().all(|arg| arg.starts_with("-psn_"))
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
mod tests {
    use super::of;
    use std::path::Path;

    #[test]
    fn the_program_of_a_bundle_names_it_and_any_other_names_none() {
        let exe = Path::new("/Applications/LLX Desk.app/Contents/MacOS/llx-desk");
        assert_eq!(of(exe), Some(Path::new("/Applications/LLX Desk.app")));
        assert_eq!(of(Path::new("/usr/local/bin/llx-desk")), None);
        assert_eq!(of(Path::new("/x/Contents/MacOS/llx-desk")), None);
        assert_eq!(of(Path::new("llx-desk")), None);
    }
}
