//! A real terminal's environment: preserve the caller's tools and credentials, not its output mode,
//! and tell the terminal what it is of the app's.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What a terminal of the app's is told about itself, over the caller's environment: the
/// folder of the app's command, first on its `PATH` (and named `LLXTERM_BIN`, for the shell to
/// put back after the person's profile sets a `PATH` of its own), and variables it is given as
/// they are. The desk says what they are; the terminal names none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Own {
    pub bin: Option<PathBuf>,
    pub vars: Vec<(String, String)>,
}

/// The variable that names the folder of the app's command in a terminal.
pub(super) const BIN: &str = "LLXTERM_BIN";

/// A PTY is not the agent's captured stdout. Programs detect this terminal's colors without
/// inherited automation overrides, and must not mistake it for the terminal that launched a
/// client. The shell's profile and explicit `NO_COLOR=1 command` still take precedence afterward.
/// Set after the caller's, `own` is the terminal's: one opened with an agent's environment says
/// which terminal it is itself, not which the agent's is.
pub(super) fn apply(program: &mut Command, vars: Option<&[(String, String)]>, own: &Own) {
    if let Some(vars) = vars {
        program.env_clear().envs(vars.iter().map(|(k, v)| (k, v)));
    }
    program
        .env_remove("NO_COLOR")
        .env_remove("FORCE_COLOR")
        .env_remove("CLICOLOR_FORCE")
        .env("TERM", super::TERM)
        .env("COLORTERM", "truecolor")
        .env("CLICOLOR", "1")
        .env("TERM_PROGRAM", "llxterm")
        .env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
    if !has_locale(vars) {
        program.env("LANG", DEFAULT_LANG);
    }
    program.envs(own.vars.iter().map(|(name, value)| (name, value)));
    if let Some(bin) = &own.bin {
        let path = match vars {
            Some(vars) => {
                (vars.iter().find(|(name, _)| name == "PATH")).map(|(_, path)| path.into())
            }
            None => std::env::var_os("PATH"),
        };
        program
            .env(BIN, bin)
            .env("PATH", first_on(bin, path.as_ref()));
    }
}

/// `path` with `bin` first, and nowhere else in it.
pub(super) fn first_on(bin: &Path, path: Option<&OsString>) -> OsString {
    let rest = path.into_iter().flat_map(std::env::split_paths);
    let all = std::iter::once(bin.to_path_buf()).chain(rest.filter(|dir| dir != bin));
    std::env::join_paths(all).unwrap_or_else(|_| bin.as_os_str().to_owned())
}

/// The locale a terminal gets on macOS when it has none, as Terminal.app sets it: an app started
/// from the Finder or the Dock has no `LANG`, and a program then reads and writes text as ASCII.
/// Elsewhere a desktop session sets one, and a name made up here may not be installed.
const DEFAULT_LANG: &str = "en_US.UTF-8";

/// Whether the program is to see a locale already (`LC_ALL`, `LC_CTYPE` or `LANG`), in `vars` when
/// it is given, else in this process's environment; always true off macOS, where none is added.
fn has_locale(vars: Option<&[(String, String)]>) -> bool {
    const NAMES: [&str; 3] = ["LC_ALL", "LC_CTYPE", "LANG"];
    if !cfg!(target_os = "macos") {
        return true;
    }
    NAMES.iter().any(|name| match vars {
        Some(vars) => vars.iter().any(|(set, _)| set == name),
        None => std::env::var_os(name).is_some(),
    })
}
