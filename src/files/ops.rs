//! What a person does to a project's files from its tree: a file or a folder made, one renamed,
//! one moved into another folder. Each names its place as the tree writes a path (`within`),
//! and none ever writes over what is there: a name in use is refused, and says so.

use std::io;
use std::path::Path;

use super::within;

/// Why nothing was done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The name is none a folder can hold.
    Name,
    /// Something of this name is there already.
    Taken(String),
    /// A folder goes into neither itself nor a folder under it.
    Inside,
    /// What was named is not there.
    Gone,
    /// The system would not: its words.
    Failed(String),
}

impl Refused {
    /// What happened, as the person is told it.
    pub fn said(&self) -> String {
        match self {
            Self::Name => "A name can’t be empty, “.” or “..”, or contain “/”.".into(),
            Self::Taken(name) => format!("The name “{name}” is already taken."),
            Self::Inside => "A folder can’t be moved into itself.".into(),
            Self::Gone => "It is no longer there.".into(),
            Self::Failed(why) => format!("{why}."),
        }
    }
}

impl From<io::Error> for Refused {
    fn from(why: io::Error) -> Self {
        if why.kind() == io::ErrorKind::NotFound {
            return Self::Gone;
        }
        // The system's words without its number.
        let said = why.to_string();
        let words = said.split(" (os error").next().unwrap_or(&said);
        Self::Failed(words.to_string())
    }
}

/// Whether `name` is one a folder can hold: a single name, not `.` or `..`, of words a person
/// can read back.
pub fn named(name: &str) -> bool {
    !name.trim().is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 255
        && !name.contains(['/', '\0', '\n', '\r'])
}

/// `name` in the folder `dir` names, as the tree writes its path.
fn under(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// An empty file (or, `folder`, a folder) named `name` made in folder `dir` of `root`: its path.
pub fn make(root: &Path, dir: &str, name: &str, folder: bool) -> Result<String, Refused> {
    if !named(name) {
        return Err(Refused::Name);
    }
    let into = within(root, dir).filter(|into| into.is_dir());
    let at = into.ok_or(Refused::Gone)?.join(name);
    let made = if folder {
        std::fs::DirBuilder::new().create(&at)
    } else {
        (std::fs::OpenOptions::new().write(true).create_new(true))
            .open(&at)
            .map(drop)
    };
    match made {
        Ok(()) => Ok(under(dir, name)),
        Err(why) if why.kind() == io::ErrorKind::AlreadyExists => {
            Err(Refused::Taken(name.to_string()))
        }
        Err(why) => Err(why.into()),
    }
}

/// The entry at `path` of `root` named `name` from now on, in the folder it is in: its path.
pub fn rename(root: &Path, path: &str, name: &str) -> Result<String, Refused> {
    if !named(name) {
        return Err(Refused::Name);
    }
    let (dir, old) = path.rsplit_once('/').unwrap_or(("", path));
    let from = (within(root, path).filter(|_| !path.is_empty())).ok_or(Refused::Gone)?;
    if old == name {
        return from
            .symlink_metadata()
            .map(|_| path.to_string())
            .map_err(Refused::from);
    }
    let to = from.with_file_name(name);
    shift(&from, &to, name)?;
    Ok(under(dir, name))
}

/// The entry at `path` of `root` moved into folder `into`, under its own name: its path. One
/// already there stays where it is.
pub fn move_into(root: &Path, path: &str, into: &str) -> Result<String, Refused> {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    let from = (within(root, path).filter(|_| !path.is_empty())).ok_or(Refused::Gone)?;
    if into == path
        || into
            .strip_prefix(path)
            .is_some_and(|rest| rest.starts_with('/'))
    {
        return Err(Refused::Inside);
    }
    let to = within(root, into).filter(|to| to.is_dir());
    let to = to.ok_or(Refused::Gone)?.join(name);
    if dir == into {
        return from
            .symlink_metadata()
            .map(|_| path.to_string())
            .map_err(Refused::from);
    }
    shift(&from, &to, name)?;
    Ok(under(into, name))
}

/// `from` is at `to` from now on, unless something else is there: then nothing moves.
fn shift(from: &Path, to: &Path, name: &str) -> Result<(), Refused> {
    let mine = from.symlink_metadata()?;
    if let Ok(there) = to.symlink_metadata() {
        // The same entry under a name that differs only as the disk does not tell names apart
        // (the letters' case): the one rename that goes where a name is.
        if !same(&mine, &there) {
            return Err(Refused::Taken(name.to_string()));
        }
        return std::fs::rename(from, to).map_err(Refused::from);
    }
    match exclusive::rename(from, to) {
        Err(why) if why.kind() == io::ErrorKind::AlreadyExists => {
            Err(Refused::Taken(name.to_string()))
        }
        done => done.map_err(Refused::from),
    }
}

/// Whether two entries read from the disk are one.
fn same(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

/// A rename that fails where its new name is in use, in one step of the system's: no entry
/// made between the look and the move is written over.
mod exclusive {
    use std::ffi::CString;
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    fn text(path: &Path) -> io::Result<CString> {
        CString::new(path.as_os_str().as_bytes())
            .map_err(|nul| io::Error::new(io::ErrorKind::InvalidInput, nul))
    }

    pub(super) fn rename(from: &Path, to: &Path) -> io::Result<()> {
        let (old, new) = (text(from)?, text(to)?);
        let done = asked(&old, &new);
        if done == 0 {
            return Ok(());
        }
        let why = io::Error::last_os_error();
        match why.raw_os_error() {
            // A disk that has no such rename: `to` was looked for a moment ago, and was free.
            Some(libc::ENOTSUP | libc::EINVAL | libc::ENOSYS) => std::fs::rename(from, to),
            _ => Err(why),
        }
    }

    #[cfg(target_os = "macos")]
    fn asked(old: &CString, new: &CString) -> i64 {
        // SAFETY: both are NUL-terminated strings that live past the call, and the flag is one
        // `renamex_np` documents.
        i64::from(unsafe { libc::renamex_np(old.as_ptr(), new.as_ptr(), libc::RENAME_EXCL) })
    }

    #[cfg(target_os = "linux")]
    fn asked(old: &CString, new: &CString) -> i64 {
        // SAFETY: `renameat2` takes two directory descriptors (the working directory's, unused
        // by absolute paths), two NUL-terminated strings that live past the call, and a flag
        // it documents.
        i64::from(unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                old.as_ptr(),
                libc::AT_FDCWD,
                new.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        })
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn asked(old: &CString, new: &CString) -> i64 {
        // SAFETY: both are NUL-terminated strings that live past the call.
        i64::from(unsafe { libc::rename(old.as_ptr(), new.as_ptr()) })
    }
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
