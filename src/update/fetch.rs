//! What is fetched for an update, by the system's `curl` as install.sh fetches it: a short
//! answer as text, a file with its progress told. Only over HTTPS, redirects included; a
//! `file://` address (a test's folder, a mirror on disk) is read as a file and nothing else.

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::run;

/// How often a download's progress is told.
const TOLD_EVERY: Duration = Duration::from_millis(250);

/// The longest a short answer is kept, in bytes: the latest release, a list of checksums.
const TEXT_MOST: usize = 2 * 1024 * 1024;

/// `curl` for `url` into `file`, within `most`: quiet, failing on the host's refusal, following
/// redirects only to where the address itself may be read from.
fn curl(url: &str, file: &Path, most: Duration) -> Command {
    let protocol = match url.starts_with("file://") {
        true => "=file",
        false => "=https",
    };
    let program = match cfg!(target_os = "macos") {
        true => "/usr/bin/curl",
        false => "curl",
    };
    let mut curl = Command::new(program);
    curl.args(["--fail", "--silent", "--show-error", "--location"])
        .args(["--proto", protocol, "--proto-redir", protocol])
        .args(["--retry", "2", "--connect-timeout", "20"])
        .args(["--max-time", &most.as_secs().max(1).to_string()])
        .args(["--user-agent", "llx-desk"])
        .arg("--output")
        .arg(file)
        .arg(url);
    curl
}

/// `url` written to `file`, within `most`; `told` hears how many bytes are there so far, as
/// they come and once at the end. A fetch that fails leaves no file.
pub(super) fn file(
    url: &str,
    file: &Path,
    most: Duration,
    told: &dyn Fn(u64),
) -> Result<(), String> {
    let _ = std::fs::remove_file(file);
    let there = || std::fs::metadata(file).map_or(0, |file| file.len());
    let last = Mutex::new(Instant::now());
    let while_running = || {
        let mut last = crate::lock(&last);
        if last.elapsed() >= TOLD_EVERY {
            told(there());
            *last = Instant::now();
        }
    };
    // Its own deadline is curl's; a little more is given for it to say so itself.
    let ran = run::within(
        curl(url, file, most),
        most + Duration::from_secs(5),
        &while_running,
    );
    if let Err(why) = ran {
        let _ = std::fs::remove_file(file);
        let why = why
            .trim_start_matches("curl failed: ")
            .trim_start_matches("curl: ");
        return Err(format!("{url} was not fetched: {why}"));
    }
    told(there());
    Ok(())
}

/// What `url` answers, as text, within `most`: fetched to a file of this call's own in `dir`,
/// read and removed.
pub(super) fn text(url: &str, dir: &Path, most: Duration) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
    let to = dir.join(format!("answer-{}", crate::token()));
    let mut fetch = curl(url, &to, most);
    fetch.args(["--max-filesize", &TEXT_MOST.to_string()]);
    let ran = run::within(fetch, most + Duration::from_secs(5), &|| {});
    let read = std::fs::read(&to);
    let _ = std::fs::remove_file(&to);
    if let Err(why) = ran {
        let why = why
            .trim_start_matches("curl failed: ")
            .trim_start_matches("curl: ");
        return Err(format!("{url} was not fetched: {why}"));
    }
    let read = read.map_err(|why| format!("{url} was not fetched: {why}"))?;
    String::from_utf8(read).map_err(|why| format!("{url} answered what is no text ({why})"))
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
pub(super) mod tests;
