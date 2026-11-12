//! A release as the app reads it: its version, its notes, the archive of this system and the
//! checksum listed for it. Pure: what is fetched is `fetch.rs`'s, what is installed
//! `install.rs`'s.

use serde::Deserialize;

/// The repository whose releases the app is updated from, as install.sh names it.
pub const REPO: &str = "localixai/localix-terminal";

/// The longest notes kept, in bytes: what a person reads of a release, not its whole history.
pub(super) const NOTES_MOST: usize = 6000;

/// A release's version: three numbers. A pre-release has none, and is never updated to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// Read from a tag or a build's own version: `v0.1.5`, `0.1.5`, `0.1.5+4089808-1794411590`
    /// (the mark after `+` is the build's, of the same version).
    pub fn read(text: &str) -> Option<Self> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let text = text.split_once('+').map_or(text, |(version, _)| version);
        let mut parts = text.split('.').map(|part| {
            let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
            digits.then(|| part.parse::<u32>().ok()).flatten()
        });
        let version = Self(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(version)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// A published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// What changed, as the release says it (Markdown).
    pub notes: String,
    /// Its files, each with its size in bytes.
    pub files: Vec<(String, u64)>,
}

impl Release {
    /// Read from the host's answer about the latest release. A draft, a pre-release, or an
    /// answer that is no release (the host's refusal, a page) is an error.
    pub(super) fn read(json: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Answer {
            tag_name: String,
            #[serde(default)]
            body: Option<String>,
            #[serde(default)]
            draft: bool,
            #[serde(default)]
            prerelease: bool,
            #[serde(default)]
            assets: Vec<Asset>,
        }
        #[derive(Deserialize)]
        struct Asset {
            name: String,
            #[serde(default)]
            size: u64,
        }
        let answer: Answer =
            serde_json::from_str(json).map_err(|why| format!("the answer is no release: {why}"))?;
        if answer.draft || answer.prerelease {
            return Err(format!("{} is not a published release", answer.tag_name));
        }
        let version = Version::read(&answer.tag_name)
            .ok_or_else(|| format!("{} is not a release's version", answer.tag_name))?;
        Ok(Self {
            version,
            notes: held(answer.body.as_deref().unwrap_or_default().trim()),
            files: (answer.assets.into_iter())
                .map(|asset| (asset.name, asset.size))
                .collect(),
        })
    }

    /// The size the release says its file `name` has; none where it has no such file.
    pub(super) fn size(&self, name: &str) -> Option<u64> {
        let found = self.files.iter().find(|(file, _)| file == name);
        found.map(|(_, size)| *size)
    }

    /// Whether it is newer than the build of version `current`. A build whose version does not
    /// read is no release's, and is never updated.
    pub fn newer_than(&self, current: &str) -> bool {
        Version::read(current).is_some_and(|current| self.version > current)
    }
}

/// `notes` held to [`NOTES_MOST`], cut at the end of a line.
fn held(notes: &str) -> String {
    if notes.len() <= NOTES_MOST {
        return notes.to_string();
    }
    let mut end = NOTES_MOST;
    while !notes.is_char_boundary(end) {
        end -= 1;
    }
    let (cut, _) = notes.split_at(end);
    let whole = cut.rsplit_once('\n').map_or(cut, |(lines, _)| lines);
    whole.trim_end().to_string()
}

/// The checksum `sums` (a release's SHA256SUMS) lists for the file named `name`: 64 hex digits.
pub(super) fn checksum(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (sum, file) = line.split_once(char::is_whitespace)?;
        let file = file.trim_start().trim_start_matches('*');
        let whole = sum.len() == 64 && sum.bytes().all(|byte| byte.is_ascii_hexdigit());
        (file == name && whole).then(|| sum.to_ascii_lowercase())
    })
}

/// The archive a release has for the system `os` on `arch` (as `std::env::consts` names them);
/// none where it has no build.
pub(super) fn archive(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("macos", "aarch64") => Some("Localix-Terminal-macos.zip"),
        ("linux", "x86_64") => Some("llxterm-x86_64-unknown-linux-gnu.tar.gz"),
        _ => None,
    }
}

/// Where releases are read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct From {
    /// The address that answers with the latest release.
    pub(super) latest: String,
    /// The address a release's files are under, each as `v0.2.0/NAME`.
    files: String,
}

impl Default for From {
    fn default() -> Self {
        Self {
            latest: format!("https://api.github.com/repos/{REPO}/releases/latest"),
            files: format!("https://github.com/{REPO}/releases/download"),
        }
    }
}

impl From {
    /// Another place, holding both under `base`: `latest`, and `download/v0.2.0/NAME`.
    pub(super) fn at(base: &str) -> Self {
        let base = base.trim_end_matches('/');
        Self {
            latest: format!("{base}/latest"),
            files: format!("{base}/download"),
        }
    }

    /// The address of the file `name` of the release `version`.
    pub(super) fn file(&self, version: Version, name: &str) -> String {
        format!("{}/v{version}/{name}", self.files)
    }
}

#[cfg(test)]
#[path = "release_tests.rs"]
mod tests;
