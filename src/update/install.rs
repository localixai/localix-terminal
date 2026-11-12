//! An update made ready and put in the app's place, as install.sh does both: the archive
//! fetched and held to the checksum its release lists, unpacked, and on macOS held to the
//! signature of the team that signs releases and to Apple's notarization; then the old app
//! moved aside and the new one moved in, the old one back if that fails.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::release::{From, Release, checksum};
use crate::fetch;

/// The Apple Developer team that signs the Mac app, as install.sh names it (a test holds the
/// two to each other).
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(
        dead_code,
        reason = "only the Mac app's signature is checked against it"
    )
)]
const TEAM_ID: &str = "64V3JT9A64";

/// How long the list of checksums and the archive may take.
const SUMS_MOST: Duration = Duration::from_secs(60);
const ARCHIVE_MOST: Duration = Duration::from_secs(30 * 60);
/// How long unpacking, a signature's check or a copy may take.
const TOOL_MOST: Duration = Duration::from_secs(5 * 60);

/// The SHA-256 of `file`, as hex.
fn sha256(file: &Path) -> Result<String, String> {
    let failed = |why: std::io::Error| format!("{}: {why}", file.display());
    let mut file_read = std::fs::File::open(file).map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut part = vec![0; 1 << 16];
    loop {
        match file_read.read(&mut part).map_err(failed)? {
            0 => break,
            read => hasher.update(&part[..read]),
        }
    }
    let hex = hasher
        .finalize()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            use std::fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    Ok(hex)
}

/// `program` with `args`, within a minute a megabyte never takes.
fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let mut command = Command::new(program);
    command.args(args);
    fetch::within(command, TOOL_MOST, &|| {})
}

/// What the archive `name` unpacks to: the app (a zip), or the program (a tarball).
pub(super) fn unpacked(name: &str) -> &'static str {
    match name.ends_with(".zip") {
        true => "Localix Terminal.app",
        false => "llxterm",
    }
}

/// The archive `name` of `release` fetched from `from` into `dir` (emptied first), held to its
/// checksum and unpacked; `told` hears the bytes fetched so far. Answers what it unpacked to.
/// Nothing of it is trusted yet: [`trusted`] says whether it is the app.
pub(super) fn stage(
    from: &From,
    release: &Release,
    name: &str,
    dir: &Path,
    told: &dyn Fn(u64),
) -> Result<PathBuf, String> {
    let made = |why: std::io::Error| format!("{}: {why}", dir.display());
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(made)?;
    let sums = fetch::text(&from.file(release.version, "SHA256SUMS"), dir, SUMS_MOST)?;
    let want =
        checksum(&sums, name).ok_or_else(|| format!("the release lists no checksum for {name}"))?;
    let archive = dir.join(name);
    fetch::file(
        &from.file(release.version, name),
        &archive,
        ARCHIVE_MOST,
        told,
    )?;
    if sha256(&archive)? != want {
        let _ = std::fs::remove_dir_all(dir);
        return Err(format!("{name} does not match its checksum"));
    }
    // Before a file of it is written: nothing is known yet of whose archive it is.
    fetch::inside(&fetch::listed(&archive, dir, TOOL_MOST)?)?;
    let into = dir.join("unpacked");
    std::fs::create_dir_all(&into).map_err(made)?;
    match name.ends_with(".zip") {
        true => run(
            "/usr/bin/ditto",
            &[
                "-x".as_ref(),
                "-k".as_ref(),
                archive.as_ref(),
                into.as_ref(),
            ],
        ),
        false => run(
            "tar",
            &[
                "-xzf".as_ref(),
                archive.as_ref(),
                "-C".as_ref(),
                into.as_ref(),
            ],
        ),
    }?;
    let _ = std::fs::remove_file(&archive);
    let staged = into.join(unpacked(name));
    match staged.exists() {
        true => Ok(staged),
        false => Err(format!("{name} holds no {}", unpacked(name))),
    }
}

/// Whether `app` is Localix Terminal as its releases are: signed by the team's Developer ID, the
/// signature whole, and notarized by Apple (macOS). The checksum comes from where the archive
/// does; this is what says whose app it is.
#[cfg(target_os = "macos")]
pub(super) fn trusted(app: &Path) -> Result<(), String> {
    let team = format!("-R=anchor apple generic and certificate leaf[subject.OU] = \"{TEAM_ID}\"");
    let signed = [
        "--verify".as_ref(),
        "--deep".as_ref(),
        "--strict".as_ref(),
        team.as_ref(),
        app.as_ref(),
    ];
    run("/usr/bin/codesign", &signed)
        .and_then(|()| {
            let assess = [
                "--assess".as_ref(),
                "--type".as_ref(),
                "execute".as_ref(),
                app.as_ref(),
            ];
            run("/usr/sbin/spctl", &assess)
        })
        .map_err(|why| format!("it is not signed and notarized as Localix Terminal is ({why})"))
}

/// Elsewhere a release is a program and its checksum: it is one that can be run.
#[cfg(not(target_os = "macos"))]
pub(super) fn trusted(program: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let runs = std::fs::metadata(program)
        .is_ok_and(|file| file.is_file() && file.permissions().mode() & 0o111 != 0);
    runs.then_some(())
        .ok_or_else(|| "it is no program".to_string())
}

/// Where what `place` held waits, beside it, until the new app runs.
pub(super) fn aside(place: &Path) -> PathBuf {
    let mut name = place.file_name().unwrap_or_default().to_os_string();
    name.push(".old");
    place.with_file_name(name)
}

/// `staged` put at `place`: what was there is moved [`aside`], and comes back if the new one
/// cannot be moved in. `staged` is first brought beside `place` (copied, where it lies on
/// another volume), so the two moves that matter are within one folder.
pub(super) fn swap(staged: &Path, place: &Path) -> Result<(), String> {
    let mut name = place.file_name().unwrap_or_default().to_os_string();
    name.push(".new");
    let beside = place.with_file_name(name);
    let _ = remove(&beside);
    if std::fs::rename(staged, &beside).is_err() {
        let copy = ["-a".as_ref(), staged.as_ref(), beside.as_ref()];
        run("/bin/cp", &copy).map_err(|why| {
            let _ = remove(&beside);
            format!("the update could not be put beside the app ({why})")
        })?;
    }
    let old = aside(place);
    let _ = remove(&old);
    let failed = |why: std::io::Error| {
        let _ = remove(&beside);
        format!("{} could not be replaced: {why}", place.display())
    };
    std::fs::rename(place, &old).map_err(failed)?;
    std::fs::rename(&beside, place).map_err(|why| {
        let _ = std::fs::rename(&old, place);
        failed(why)
    })
}

/// A file or a folder removed.
pub(super) fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path)?.is_dir() {
        true => std::fs::remove_dir_all(path),
        false => std::fs::remove_file(path),
    }
}

#[cfg(test)]
#[path = "install_tests.rs"]
pub(super) mod tests;
