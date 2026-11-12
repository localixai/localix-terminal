//! Where this app is, whether it is one the app itself replaces, and how it is started again
//! once it was.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Whether the person may write in `dir`.
fn writable(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `path` is a NUL-terminated string that lives through the call.
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

/// Where this app is: its bundle (macOS), or the program where install.sh puts it. An error
/// says why the app does not replace it: a build that is no release, a place that does not
/// last or cannot be written.
pub(super) fn of_this_app() -> Result<PathBuf, String> {
    let place = installed()?;
    match place.parent().is_some_and(writable) {
        true => Ok(place),
        false => Err(format!(
            "Localix Terminal cannot replace itself in {}: you may not write there.",
            place.parent().unwrap_or(&place).display()
        )),
    }
}

/// What the app says of a build it does not update.
const NOT_A_RELEASE: &str = "This build is not a release: it is updated the way it was made.";

#[cfg(target_os = "macos")]
fn installed() -> Result<PathBuf, String> {
    let app = crate::bundle::app().ok_or(NOT_A_RELEASE)?;
    if !crate::bundle::in_place() {
        return Err(
            "Move Localix Terminal to Applications first: it runs from a place that does not last."
                .into(),
        );
    }
    // A build made on this Mac is signed by no one: it is never replaced by a release.
    super::install::trusted(&app).map_err(|_unsigned| NOT_A_RELEASE)?;
    Ok(app)
}

#[cfg(not(target_os = "macos"))]
fn installed() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|_unknown| NOT_A_RELEASE)?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    // Where install.sh puts the program, and nowhere else.
    let data = std::env::var_os("XDG_DATA_HOME").filter(|dir| !dir.is_empty());
    let data = data
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let installed = data.map(|data| data.join("llxterm/llxterm"));
    match installed.and_then(|installed| std::fs::canonicalize(installed).ok()) {
        Some(installed) if installed == exe => Ok(exe),
        _ => Err(NOT_A_RELEASE.into()),
    }
}

/// The app put in place, to be started again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Again {
    /// Where the app is.
    pub(super) place: PathBuf,
    /// Where the app it replaced waits, to be removed once this process ended.
    pub(super) old: PathBuf,
}

impl Again {
    /// What waits for this process to end, opens the new app and removes the old one: a shell
    /// of its own, which outlives the app. Run as the app ends.
    pub fn start(&self) -> Result<(), String> {
        use std::os::unix::process::CommandExt;
        // The old app goes once the new one opened; one that does not open gives its place
        // back to the old, which opens as it did.
        let open = match cfg!(target_os = "macos") {
            true => {
                r#"if /usr/bin/open -n "$3"; then rm -rf "$2"; else rm -rf "$3"; mv "$2" "$3"; exec /usr/bin/open -n "$3"; fi"#
            }
            false => r#"rm -rf "$2"; exec "$3" open"#,
        };
        let script = format!(r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done; {open}"#);
        Command::new("/bin/sh")
            .args(["-c", &script, "llxterm-update"])
            .arg(std::process::id().to_string())
            .arg(&self.old)
            .arg(&self.place)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map(|_| ())
            .map_err(|why| format!("Localix Terminal was updated; open it again ({why})."))
    }
}
