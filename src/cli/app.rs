//! llxterm opened as an app: Localix Terminal.app from the Finder, Spotlight or the Dock (macOS), or
//! `llxterm open` (the desktop entry elsewhere). It puts what is always on in place (the
//! `llxterm` command, setup.rs) and brings what the person turned on to this version, then is
//! the server itself, its window brought to the person at once: with no tab in it yet, their
//! shell in their home. When a server of this build runs already (one a client started), the
//! window is asked of it instead; one of another build is stopped, and the app takes its place:
//! the person opened this app.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::{Serving, serving};
use crate::client::{Start, Trouble, ask};

/// The app opened: set up, then the server run with its window brought to the person. What
/// happened goes to `app.log` in the desk's home, where a person who wonders can read it.
pub(super) fn run() -> u8 {
    crate::server::log_to_home();
    let mut said = vec![];
    // What is written into the person's tools names this program: not one that is on a disk image
    // or in a download's temporary place, which is gone at eject or restart.
    let in_place = crate::bundle::in_place();
    if !in_place {
        said.push(
            "not set up: move Localix Terminal to Applications, and open it from there".into(),
        );
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match bind_taking_over(&mut said) {
        Ok(crate::server::Binding::Bound(bound)) => {
            said.push("the app serves".into());
            note(&said);
            // The person's own programs run (`claude`, their login shell, which may take
            // seconds): not before the window.
            if in_place {
                keep_well();
            }
            let serve = Serving {
                headless: false,
                windowless: false,
                opened: true,
            };
            serving(bound, serve)
        }
        Ok(crate::server::Binding::Running) => {
            let code = present(&mut said, home.as_deref());
            // After the window: the command may wait for the person's login shell. The server
            // that runs looks after the rest (its doctor).
            if in_place {
                said.extend(crate::setup::opened());
            }
            note(&said);
            code
        }
        Err(why) => {
            said.push(format!("the app could not serve: {why}"));
            note(&said);
            1
        }
    }
}

/// `llxterm open`: on macOS the system opens Localix Terminal.app, as the Dock does, and the command
/// is done; the app is not this terminal's (closing the terminal leaves it running). Elsewhere,
/// or run from no app (a build in the tree), this process is the app.
pub(super) fn open() -> u8 {
    #[cfg(target_os = "macos")]
    if let Some(bundle) = crate::bundle::app() {
        return match ask_open(&bundle) {
            Ok(()) => super::OK,
            Err(why) => {
                crate::out::warn(&format!(
                    "llxterm: {} did not open: {why}",
                    bundle.display()
                ));
                super::FAILED
            }
        };
    }
    run()
}

/// The system asked to open `bundle`, as the Dock does (with this caller's places for the app's
/// files, as a call that starts it): done once `open` says so, within a few seconds.
#[cfg(target_os = "macos")]
fn ask_open(bundle: &Path) -> Result<(), String> {
    let mut open = std::process::Command::new("/usr/bin/open");
    crate::client::keep_places(&mut open);
    let child = open.arg(bundle).spawn().map_err(|why| why.to_string())?;
    match crate::client::reap(child)?.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(status.to_string()),
        Err(_) => Err("the system did not answer".into()),
    }
}

/// How many times a server of another build is stopped for this one, when another starts again
/// in its place before this one could (a client of that build that starts its own).
const TAKEOVERS: usize = 3;

/// The lock and the socket, taken from a server of another build when one holds them: it is
/// asked to stop, and this app binds as soon as it lets go.
fn bind_taking_over(said: &mut Vec<String>) -> std::io::Result<crate::server::Binding> {
    for _ in 0..TAKEOVERS {
        let bound = crate::server::bind()?;
        let crate::server::Binding::Running = bound else {
            return Ok(bound);
        };
        let notes: crate::client::Notes = std::sync::Arc::new(|_, _| {});
        let other = match crate::client::Client::connect(Start::Never, &notes) {
            Ok(Some(client)) if client.server.build != crate::server::build() => client,
            // This build's, or none that answers: it is asked for the window.
            _ => return Ok(crate::server::Binding::Running),
        };
        said.push(format!(
            "a server of build {} runs: stopped, for this app's",
            other.server.build
        ));
        let _ = other.request("app/quit", json!({}));
        drop(other);
        if let Some(bound) = freed()? {
            return Ok(crate::server::Binding::Bound(bound));
        }
        // One that does not quit when asked (a build that names quitting otherwise) is ended.
        if let Some(pid) = crate::server::holder() {
            said.push(format!("it did not quit when asked: process {pid} ended"));
            // SAFETY: a signal to the process that wrote the lock, in this user's own directory.
            let _ = unsafe { libc::kill(pid, libc::SIGTERM) };
        }
        if let Some(bound) = freed()? {
            return Ok(crate::server::Binding::Bound(bound));
        }
    }
    Ok(crate::server::Binding::Running)
}

/// The lock and the socket once the other server lets go of them, within a few seconds.
fn freed() -> std::io::Result<Option<crate::server::Bound>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if let crate::server::Binding::Bound(bound) = crate::server::bind()? {
            return Ok(Some(bound));
        }
        crate::backoff(std::time::Duration::from_millis(20));
    }
    Ok(None)
}

/// The window asked of the server that runs already.
fn present(said: &mut Vec<String>, home: Option<&Path>) -> u8 {
    let home = home.unwrap_or_else(|| Path::new("/"));
    let caller = crate::client::Caller {
        project: home.to_path_buf(),
        agent: None,
    };
    let params = caller.with(json!({
        "cwd": home.display().to_string(),
        "env": crate::client::environment(),
    }));
    match ask(Start::Never, "window/present", &params) {
        Ok(answer) => {
            said.push(
                answer.value["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            );
            0
        }
        Err(trouble) => {
            let why = match trouble {
                Trouble::Refused(why) | Trouble::Lost(why) => why,
                Trouble::None => "no server answered".into(),
            };
            // Where the person can see it from `llxterm open`; the log has it for the icon.
            crate::out::warn(&format!("llxterm: the window did not open: {why}"));
            said.push(format!("the window did not open: {why}"));
            1
        }
    }
}

/// `llxterm open DIR`: the window on the project at `dir` (from the current directory), as it
/// was left, or with what it had open; the app started, out of sight, when it is not running.
pub(super) fn open_in(dir: &str) -> u8 {
    let folder = std::env::current_dir().unwrap_or_default().join(dir);
    if !folder.is_dir() {
        crate::out::warn(&format!("llxterm: {} is not a folder", folder.display()));
        return super::UNREACHABLE;
    }
    let caller = crate::client::Caller::at(&folder, None);
    let params = caller.with(json!({
        "cwd": folder.display().to_string(),
        "env": crate::client::environment(),
        "go": true,
    }));
    match ask(Start::Spawn, "window/present", &params) {
        Ok(answer) => {
            crate::out::say(answer.value["text"].as_str().unwrap_or_default());
            super::OK
        }
        Err(Trouble::Refused(why)) => {
            crate::out::warn(&format!("llxterm: {why}"));
            super::FAILED
        }
        Err(Trouble::Lost(why)) => {
            crate::out::warn(&format!("llxterm: {why}"));
            super::UNREACHABLE
        }
        Err(Trouble::None) => super::UNREACHABLE,
    }
}

/// The doctor started (setup/doctor.rs): the integrations put right now and every hour, what
/// it did written to the log.
pub(super) fn keep_well() {
    if let Err(why) = crate::setup::keep_well(note) {
        note(&[format!(
            "the agents are not looked after (no thread: {why})"
        )]);
    }
}

/// `said` appended to the app's log, each line stamped.
pub(super) fn note(said: &[String]) {
    let file = crate::paths::log();
    let opened = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file);
    if let Ok(mut file) = opened {
        let stamp = crate::now();
        for line in said {
            let _ = writeln!(file, "{stamp} {line}");
        }
    }
}
