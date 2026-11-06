//! Where the app lives. Its lock and its socket are at one fixed place per user, found the
//! same by every process of that user whatever its environment (`LLX_DESK_HOME` and `TMPDIR`
//! included): there is one app, and every client must reach it. Its files (log, pictures,
//! comments) go to the home, which `LLX_DESK_HOME` moves and which is the same directory
//! otherwise.

use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The directory of the lock and the socket: `llx-desk-<uid>` in the user's temporary
/// directory as the system names it (not `$TMPDIR`, which a sandbox or a wrapper may change).
/// A test build takes it from `LLX_DESK_TEST_DIR` and from nowhere else.
pub fn runtime() -> PathBuf {
    if cfg!(feature = "test-server") {
        // Without its directory a test build has none: nothing is created there, and nothing
        // found is the person's app.
        return std::env::var_os("LLX_DESK_TEST_DIR")
            .filter(|dir| !dir.is_empty())
            .map_or_else(
                || PathBuf::from("/nonexistent/llx-desk-a-test-build-needs-LLX_DESK_TEST_DIR"),
                PathBuf::from,
            );
    }
    // SAFETY: getuid takes nothing and cannot fail.
    let uid = unsafe { libc::getuid() };
    under(&user_temp(), uid)
}

/// `base/llx-desk-<uid>`.
pub(crate) fn under(base: &Path, uid: u32) -> PathBuf {
    base.join(format!("llx-desk-{uid}"))
}

/// The per-user temporary directory: on macOS `confstr` names it (what `TMPDIR` is by default),
/// elsewhere `/tmp`.
fn user_temp() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let mut buf = vec![0u8; 1024];
        // SAFETY: confstr writes at most `buf.len()` bytes, the last a NUL, into `buf`.
        let len = unsafe {
            libc::confstr(
                libc::_CS_DARWIN_USER_TEMP_DIR,
                buf.as_mut_ptr().cast(),
                buf.len(),
            )
        };
        if (1..=buf.len()).contains(&len) {
            buf.truncate(len - 1);
            while buf.len() > 1 && buf.last() == Some(&b'/') {
                buf.pop();
            }
            return PathBuf::from(std::ffi::OsStr::from_bytes(&buf));
        }
    }
    PathBuf::from("/tmp")
}

/// The variables that move the desk's files: a server started for a caller (the app, through
/// LaunchServices, which gives it none of the caller's) is given these, so both find the same.
pub const MOVED_BY: [&str; 3] = ["LLX_DESK_HOME", "LLX_DESK_DATA", "XDG_CONFIG_HOME"];

/// `$LLX_DESK_HOME`, else the runtime directory: where the log, the pictures and the
/// comments go.
pub fn home() -> PathBuf {
    std::env::var_os("LLX_DESK_HOME")
        .filter(|home| !home.is_empty())
        .map_or_else(runtime, PathBuf::from)
}

/// What the desk keeps across restarts: `LLX_DESK_DATA`, else `~/.local/share/llx-desk`. A test
/// build keeps it in its own directory.
pub fn data() -> PathBuf {
    if cfg!(feature = "test-server") {
        return runtime();
    }
    if let Some(dir) = std::env::var_os("LLX_DESK_DATA").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME").map_or_else(runtime, |home| {
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("llx-desk")
    })
}

/// The person's settings and secrets: `$XDG_CONFIG_HOME/llx-desk`, else `~/.config/llx-desk`.
pub fn config() -> Option<PathBuf> {
    config_in(
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

fn config_in(config: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    config
        .filter(|dir| !dir.as_os_str().is_empty())
        .or_else(|| home.map(|home| home.join(".config")))
        .map(|dir| dir.join("llx-desk"))
}

/// The app's database, in [`data`].
pub fn database() -> PathBuf {
    data().join("llx-desk.sqlite")
}

pub fn socket() -> PathBuf {
    runtime().join("desk.sock")
}

pub fn lock() -> PathBuf {
    runtime().join("desk.lock")
}

pub fn shots() -> PathBuf {
    home().join("shots")
}

/// The app's one log, in the home: what it said while serving, and what it did when opened.
pub fn log() -> PathBuf {
    home().join("app.log")
}

/// The runtime directory, made with mode 0700 and checked to be the user's own: a directory
/// someone else made at this predictable name is refused, never used.
pub fn ensure_runtime() -> io::Result<PathBuf> {
    let dir = runtime();
    own_private_dir(&dir)?;
    Ok(dir)
}

fn own_private_dir(dir: &Path) -> io::Result<()> {
    create_private(dir)?;
    let found = std::fs::symlink_metadata(dir)?;
    // SAFETY: getuid takes nothing and cannot fail.
    if !found.is_dir() || found.uid() != unsafe { libc::getuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a directory of this user's own", dir.display()),
        ));
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

/// The home directory, made with mode 0700 (and set to it when it was there already).
pub fn ensure_home() -> io::Result<PathBuf> {
    let home = home();
    create_private(&home)?;
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700))?;
    Ok(home)
}

fn create_private(dir: &Path) -> io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
