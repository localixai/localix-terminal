//! The first thing a run does after LLX Desk 0.1.6, before anything opens the database or the
//! settings: on macOS the app that 0.1.6's updater left as `LLX Desk.app` takes its own name
//! and is opened from there, and 0.1.6's files are carried to this app's names
//! (`paths::carry_over`). An LLX Desk that still serves is stopped first when the app starts
//! (opened, or `serve`), as a server of another build is (`app.rs`): its data must not move
//! under it. A command run beside it carries nothing, and leaves it running.

use std::time::{Duration, Instant};

/// What 0.1.6 left put right, for a run of `args` (`app`: the program opened as the app). An
/// exit code when this run is done: the app was opened again under its new name.
pub(super) fn begin(args: &[String], app: bool) -> Option<u8> {
    let first = args.first().map(String::as_str);
    let launch = app || first == Some("serve");
    let asks_only = matches!(
        first,
        None | Some("help" | "-h" | "--help" | "-V" | "--version")
    );
    if !launch && asks_only {
        return None;
    }
    let mut said = vec![];
    #[cfg(target_os = "macos")]
    if launch && let Some(code) = renamed(args, app, &mut said) {
        noted(&said);
        return Some(code);
    }
    if let Some(pid) = crate::paths::former_server() {
        if !launch {
            return None;
        }
        if !stopped(pid, &mut said) {
            noted(&said);
            return None;
        }
    }
    said.extend(crate::paths::carry_over());
    noted(&said);
    None
}

/// `said` written to the app's log, when there is anything.
fn noted(said: &[String]) {
    if !said.is_empty() {
        let _ = crate::paths::ensure_home();
        super::app::note(said);
    }
}

/// LLX Desk's server (process `pid`) stopped, and its lock let go within a few seconds;
/// whether it was.
fn stopped(pid: libc::pid_t, said: &mut Vec<String>) -> bool {
    said.push(format!(
        "LLX Desk serves (process {pid}): stopped, so that its data comes to this app"
    ));
    // SAFETY: a signal to the process that wrote LLX Desk's lock, in this user's own directory.
    let _ = unsafe { libc::kill(pid, libc::SIGTERM) };
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if crate::paths::former_server().is_none() {
            return true;
        }
        crate::backoff(Duration::from_millis(50));
    }
    said.push("LLX Desk did not stop: its data stays where it is until it has quit".into());
    false
}

/// The app, when it is a release under 0.1.6's name in a place that lasts, renamed
/// `Localix Terminal.app` beside where it is and opened again from there with what this run was
/// given; an exit code once that one opened. One that does not open is renamed back, and this
/// run goes on as it was.
#[cfg(target_os = "macos")]
fn renamed(args: &[String], app: bool, said: &mut Vec<String>) -> Option<u8> {
    if cfg!(debug_assertions) || cfg!(feature = "test-server") || !crate::bundle::in_place() {
        return None;
    }
    let from = crate::bundle::app()?;
    let to = crate::bundle::renamed(&from)?;
    let (from_shown, to_shown) = (from.display(), to.display());
    if let Err(why) = std::fs::rename(&from, &to) {
        said.push(format!("{from_shown} keeps its name: {why}"));
        return None;
    }
    let mut open = std::process::Command::new("/usr/bin/open");
    open.arg("-n");
    crate::client::keep_places(&mut open);
    open.arg(&to);
    // The app opened is given nothing but what the system adds; `serve` is given its flags.
    if !app {
        open.arg("--args").args(args);
    }
    let opened = (open.spawn().map_err(|why| why.to_string()))
        .and_then(crate::client::reap)
        .and_then(|exited| {
            exited
                .recv_timeout(Duration::from_secs(10))
                .map_err(|_late| "the system did not answer".to_string())
        });
    match opened {
        Ok(status) if status.success() => {
            said.push(format!(
                "{from_shown} is now {to_shown}, and opened from there"
            ));
            Some(0)
        }
        failed => {
            let why = failed.map_or_else(|why| why, |status| status.to_string());
            let back = std::fs::rename(&to, &from).map_or_else(
                |undone| format!("; it stays {to_shown}: {undone}"),
                |()| String::new(),
            );
            said.push(format!(
                "{to_shown} did not open ({why}): the app runs as {from_shown}{back}"
            ));
            None
        }
    }
}
