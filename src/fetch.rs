//! What is fetched over the network, by the system's `curl`, and opened, by its `tar`: a short
//! answer as text, asked again only for what changed since its ETag; a file with its progress
//! told; an archive unpacked inside its folder (`unpack.rs`). Only over HTTPS, redirects
//! included; a `file://` address (a test's folder, a mirror on disk) is read as a file and
//! nothing else. Every system tool runs within a deadline (`run.rs`), the updater's own too.

mod run;
mod unpack;

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub use run::{within, within_into};
pub use unpack::{inside, listed, untar};

/// How often a download's progress is told.
const TOLD_EVERY: Duration = Duration::from_millis(250);

/// The longest a short answer is kept unless asked otherwise, in bytes: the latest release, a
/// list of checksums, a catalogue's index.
pub const TEXT_MOST: usize = 2 * 1024 * 1024;

/// What a short answer was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    /// The same as the one the ETag given was of: nothing was sent again.
    Same,
    /// What the host answers, with the ETag it gave it, when it gave one.
    New { text: String, etag: Option<String> },
}

/// How a short answer is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ask<'a> {
    /// The longest the whole fetch may take.
    pub within: Duration,
    /// The longest answer kept, in bytes: a longer one is an error.
    pub most: usize,
    /// The ETag of the answer last read: the host answers only if it changed.
    pub etag: Option<&'a str>,
}

impl Ask<'_> {
    /// An answer of at most [`TEXT_MOST`], asked for whole.
    pub fn within(within: Duration) -> Self {
        Self {
            within,
            most: TEXT_MOST,
            etag: None,
        }
    }
}

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
        .args(["--user-agent", "llxterm"])
        .arg("--output")
        .arg(file)
        .arg(url);
    curl
}

/// An ETag as it may be sent back: printable, on one line. Anything else is not sent.
fn sendable(etag: &str) -> bool {
    !etag.is_empty() && etag.len() <= 512 && etag.bytes().all(|byte| (0x20..0x7f).contains(&byte))
}

/// `curl` for the short answer `ask` describes: into `body`, the ETag the host gives saved to
/// `tag`, the status code written out where the command's output goes.
fn asking(url: &str, body: &Path, tag: &Path, ask: &Ask<'_>) -> Command {
    let mut fetch = curl(url, body, ask.within);
    fetch
        .args(["--compressed", "--max-filesize", &ask.most.to_string()])
        .args(["--write-out", "%{http_code}"])
        .arg("--etag-save")
        .arg(tag);
    if let Some(etag) = ask.etag.filter(|etag| sendable(etag)) {
        fetch.args(["--header", &format!("If-None-Match: {etag}")]);
    }
    fetch
}

/// Why `url` was not fetched, in the words curl said it in.
fn not_fetched(url: &str, why: &str) -> String {
    let why = why
        .trim_start_matches("curl failed: ")
        .trim_start_matches("curl: ");
    format!("{url} was not fetched: {why}")
}

/// What `url` answers, as `ask` says, fetched to files of this call's own in `dir`, read and
/// removed.
pub fn changed(url: &str, dir: &Path, ask: &Ask<'_>) -> Result<Fetched, String> {
    std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
    let at = dir.join(format!("answer-{}", crate::token()));
    let [body, tag, code] = ["", ".etag", ".code"].map(|end| {
        let mut name = at.clone().into_os_string();
        name.push(end);
        std::path::PathBuf::from(name)
    });
    let out = std::fs::File::create(&code).map_err(|why| not_fetched(url, &why.to_string()))?;
    let ran = within_into(
        asking(url, &body, &tag, ask),
        out.into(),
        ask.within + Duration::from_secs(5),
        &|| {},
    );
    let read = std::fs::read(&body);
    let etag = std::fs::read_to_string(&tag).unwrap_or_default();
    let status = std::fs::read_to_string(&code).unwrap_or_default();
    for file in [&body, &tag, &code] {
        let _ = std::fs::remove_file(file);
    }
    ran.map_err(|why| not_fetched(url, &why))?;
    if status.trim() == "304" {
        return Ok(Fetched::Same);
    }
    let read = read.map_err(|why| not_fetched(url, &why.to_string()))?;
    let text =
        String::from_utf8(read).map_err(|why| format!("{url} answered what is no text ({why})"))?;
    let etag = Some(etag.trim().to_string()).filter(|etag| sendable(etag));
    Ok(Fetched::New { text, etag })
}

/// What `url` answers, as text of at most [`TEXT_MOST`], within `most`.
pub fn text(url: &str, dir: &Path, most: Duration) -> Result<String, String> {
    match changed(url, dir, &Ask::within(most))? {
        Fetched::New { text, .. } => Ok(text),
        Fetched::Same => Err(format!("{url} answered that nothing changed, unasked")),
    }
}

/// `url` written to `file`, within `most`; `told` hears how many bytes are there so far, as
/// they come and once at the end. A fetch that fails leaves no file.
pub fn file(url: &str, file: &Path, most: Duration, told: &dyn Fn(u64)) -> Result<(), String> {
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
    let ran = within(
        curl(url, file, most),
        most + Duration::from_secs(5),
        &while_running,
    );
    if let Err(why) = ran {
        let _ = std::fs::remove_file(file);
        return Err(not_fetched(url, &why));
    }
    told(there());
    Ok(())
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
pub(crate) mod tests;
