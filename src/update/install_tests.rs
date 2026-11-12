//! Tests of `update::install`, against a folder on disk standing for the releases' host.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::*;
use crate::fetch::tests::folder;

/// A release of `version` laid out under `dir` as the host serves it, its archive `name` a
/// tarball of a program that says `says`. Answers where to read it from, and the release.
pub(in crate::update) fn published(
    dir: &Path,
    version: &str,
    name: &str,
    says: &str,
) -> (From, Release) {
    let files = dir.join("download").join(format!("v{version}"));
    std::fs::create_dir_all(&files).unwrap();
    let made = dir.join(format!("made-{version}"));
    std::fs::create_dir_all(&made).unwrap();
    std::fs::write(made.join("llxterm"), format!("#!/bin/sh\necho {says}\n")).unwrap();
    let archive = files.join(name);
    let packed = [
        "-czf".as_ref(),
        archive.as_os_str(),
        "-C".as_ref(),
        made.as_os_str(),
        "llxterm".as_ref(),
    ];
    run("tar", &packed).unwrap();
    let sum = sha256(&files.join(name)).unwrap();
    std::fs::write(files.join("SHA256SUMS"), format!("{sum}  {name}\n")).unwrap();
    let size = std::fs::metadata(files.join(name)).unwrap().len();
    let latest = serde_json::json!({
        "tag_name": format!("v{version}"),
        "body": format!("## Added\n\n- It says {says}."),
        "assets": [{"name": name, "size": size}],
    });
    std::fs::write(dir.join("latest"), latest.to_string()).unwrap();
    let from = From::at(&format!("file://{}", dir.display()));
    let release = Release::read(&latest.to_string()).unwrap();
    (from, release)
}

const NAME: &str = "llxterm-x86_64-unknown-linux-gnu.tar.gz";

#[test]
fn an_update_is_fetched_checked_and_unpacked() {
    let dir = folder("stage");
    let (from, release) = published(&dir, "0.2.0", NAME, "new");
    let heard = Mutex::new(Vec::new());
    let told = |so_far: u64| heard.lock().unwrap().push(so_far);
    let staged = stage(&from, &release, NAME, &dir.join("staging"), &told).unwrap();
    assert_eq!(staged, dir.join("staging/unpacked/llxterm"));
    assert!(
        std::fs::read_to_string(&staged)
            .unwrap()
            .contains("echo new")
    );
    // All of it was told, and the archive is not kept beside what it held.
    assert_eq!(heard.lock().unwrap().last().copied(), release.size(NAME));
    assert!(!dir.join("staging").join(NAME).exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_archive_that_is_not_what_its_release_lists_is_not_unpacked() {
    let dir = folder("tampered");
    let (from, release) = published(&dir, "0.2.0", NAME, "new");
    let files = dir.join("download/v0.2.0");
    // Another archive under the same name.
    let (_, _) = published(&dir.join("other"), "0.2.0", NAME, "evil");
    std::fs::copy(
        dir.join("other/download/v0.2.0").join(NAME),
        files.join(NAME),
    )
    .unwrap();
    let staging = dir.join("staging");
    let refused = stage(&from, &release, NAME, &staging, &|_| {}).unwrap_err();
    assert_eq!(refused, format!("{NAME} does not match its checksum"));
    assert!(!staging.exists());
    // A release that lists no checksum for it is refused before the archive is asked for.
    std::fs::write(files.join("SHA256SUMS"), "ffff  another-file\n").unwrap();
    let unlisted = stage(&from, &release, NAME, &staging, &|_| {}).unwrap_err();
    assert_eq!(
        unlisted,
        format!("the release lists no checksum for {NAME}")
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// An app folder at `at` whose one file says `says`.
fn app(at: &Path, says: &str) -> PathBuf {
    std::fs::create_dir_all(at.join("Contents")).unwrap();
    std::fs::write(at.join("Contents/says"), says).unwrap();
    at.to_path_buf()
}

fn says(app: &Path) -> String {
    std::fs::read_to_string(app.join("Contents/says")).unwrap()
}

#[test]
fn the_new_app_takes_the_old_one_s_place_and_the_old_one_waits_aside() {
    let dir = folder("swap");
    let place = app(&dir.join("Applications/Localix Terminal.app"), "old");
    let staged = app(&dir.join("staging/unpacked/Localix Terminal.app"), "new");
    swap(&staged, &place).unwrap();
    assert_eq!(says(&place), "new");
    assert_eq!(
        aside(&place),
        dir.join("Applications/Localix Terminal.app.old")
    );
    assert_eq!(says(&aside(&place)), "old");
    assert!(!staged.exists());
    // An update after it: what waited aside before is gone, not in the way.
    let staged = app(&dir.join("staging/unpacked/Localix Terminal.app"), "newer");
    swap(&staged, &place).unwrap();
    assert_eq!(
        (says(&place), says(&aside(&place))),
        ("newer".into(), "new".into())
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_update_that_cannot_be_put_in_place_leaves_the_app_as_it_was() {
    let dir = folder("swap-failed");
    let place = app(&dir.join("Applications/Localix Terminal.app"), "old");
    let missing = dir.join("staging/unpacked/Localix Terminal.app");
    assert!(swap(&missing, &place).is_err());
    assert_eq!(says(&place), "old");
    assert!(!aside(&place).exists());
    assert!(!dir.join("Applications/Localix Terminal.app.new").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(target_os = "macos")]
#[test]
fn an_app_no_one_signed_is_not_trusted() {
    let dir = folder("unsigned");
    let unsigned = app(&dir.join("Localix Terminal.app"), "who knows");
    let why = trusted(&unsigned).unwrap_err();
    assert!(
        why.starts_with("it is not signed and notarized as Localix Terminal is ("),
        "{why}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_app_is_held_to_the_team_and_the_repository_install_sh_names() {
    let script = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/install.sh"));
    let script = script.unwrap();
    let named = |key: &str| {
        (script.lines())
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
            .map(|value| value.trim_matches('"').to_string())
    };
    assert_eq!(named("TEAM_ID").as_deref(), Some(TEAM_ID));
    assert_eq!(named("REPO").as_deref(), Some(crate::update::REPO));
}
