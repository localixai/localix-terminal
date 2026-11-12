//! Tests of `catalog::archive`: a repository's archive made here with the system's `tar`, and
//! fetched through a network that copies it from disk.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::*;
use crate::catalog::source::tests::Fake;
use crate::testing::Scratch;

/// A repository's archive at `to`, as GitHub makes one: `files` (and what `dir` holds there
/// already) under one folder, `top`.
pub(crate) fn archive_of(dir: &Path, top: &str, files: &[(&str, &str)], to: &Path) {
    for (name, text) in files {
        let path = dir.join(top).join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let mut tar = Command::new("tar");
    tar.arg("-czf").arg(to).arg("-C").arg(dir).arg(top);
    crate::fetch::within(tar, Duration::from_secs(30), &|| {}).unwrap();
}

const URL: &str = "https://codeload.github.com/anthropics/skills/tar.gz/abc123";

#[test]
fn an_archive_is_unpacked_once_without_its_top_folder_and_reused_after() {
    let scratch = Scratch::new();
    let tarball = scratch.path().join("repo.tar.gz");
    archive_of(
        &scratch.path().join("made"),
        "skills-abc123",
        &[("skills/pdf/SKILL.md", "---\nname: pdf\n---\n")],
        &tarball,
    );
    let cache = scratch.path().join("cache");
    let net = Fake::default().with_file(URL, tarball);
    let folder = unpacked(&net, &cache, "anthropics/skills", "abc123").unwrap();
    assert_eq!(
        std::fs::read_to_string(folder.join("skills/pdf/SKILL.md")).unwrap(),
        "---\nname: pdf\n---\n"
    );
    let again = unpacked(&net, &cache, "anthropics/skills", "abc123").unwrap();
    assert_eq!(again, folder);
    assert_eq!(net.fetched.lock().unwrap().len(), 1);
    // Nothing but the folder is left beside it.
    let beside: Vec<_> = std::fs::read_dir(folder.parent().unwrap())
        .unwrap()
        .collect();
    assert_eq!(beside.len(), 1);
}

#[test]
fn an_archive_with_a_link_out_of_its_folder_is_refused_and_leaves_nothing() {
    let scratch = Scratch::new();
    let made = scratch.path().join("made");
    std::fs::create_dir_all(made.join("skills-abc123/skills")).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", made.join("skills-abc123/skills/out")).unwrap();
    let tarball = scratch.path().join("repo.tar.gz");
    archive_of(&made, "skills-abc123", &[], &tarball);
    let cache = scratch.path().join("cache");
    let net = Fake::default().with_file(URL, tarball);
    let why = unpacked(&net, &cache, "anthropics/skills", "abc123").unwrap_err();
    assert!(why.contains("outside its folder"), "{why}");
    let kept = folder(&cache, "anthropics/skills", "abc123").unwrap();
    assert_eq!(
        std::fs::read_dir(kept.parent().unwrap()).unwrap().count(),
        0
    );
}

#[test]
fn a_repository_or_revision_that_could_name_another_folder_is_refused() {
    let cache = Path::new("/cache");
    for (repo, rev) in [
        ("anthropics", "main"),
        ("../x/y", "main"),
        ("a/b/c", "main"),
        ("a/..", "main"),
        ("a/b", ".."),
        ("a/b", "feature/x"),
        ("a/b", ""),
    ] {
        assert!(folder(cache, repo, rev).is_err(), "{repo} {rev}");
    }
    assert_eq!(
        folder(cache, "anthropics/skills", "v1.0").unwrap(),
        Path::new("/cache/archives/anthropics/skills/v1.0")
    );
}

#[test]
fn only_the_revision_kept_stays_in_the_cache() {
    let scratch = Scratch::new();
    let cache = scratch.path();
    for rev in ["old", "new", ".new-unpacking"] {
        let at = cache.join("archives/anthropics/skills").join(rev);
        std::fs::create_dir_all(at).unwrap();
    }
    keep_only(cache, "anthropics/skills", "new");
    let mut left: Vec<String> = std::fs::read_dir(cache.join("archives/anthropics/skills"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(left, [".new-unpacking", "new"]);
}
