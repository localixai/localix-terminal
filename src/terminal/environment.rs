//! A real terminal's environment: preserve the caller's tools and credentials, not its output mode.

use std::process::Command;

/// A PTY is not the agent's captured stdout. Programs detect this terminal's colors without
/// inherited automation overrides, and must not mistake it for the terminal that launched a
/// client. The shell's profile and explicit `NO_COLOR=1 command` still take precedence afterward.
pub(super) fn apply(program: &mut Command, vars: Option<&[(String, String)]>) {
    if let Some(vars) = vars {
        program.env_clear().envs(vars.iter().map(|(k, v)| (k, v)));
    }
    program
        .env_remove("NO_COLOR")
        .env_remove("FORCE_COLOR")
        .env_remove("CLICOLOR_FORCE")
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .env("CLICOLOR", "1")
        .env("TERM_PROGRAM", "llxterm")
        .env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
    if !has_locale(vars) {
        program.env("LANG", DEFAULT_LANG);
    }
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
