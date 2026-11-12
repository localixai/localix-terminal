//! Tests of `fetch::unpack`: tarballs made here with the system's `tar`, opened into folders of
//! the test's own.

use std::path::Path;
use std::process::Command;

use super::*;
use crate::fetch::tests::folder;

const MOST: Duration = Duration::from_secs(30);

/// A tarball at `to` of the folder `top` under `dir`, as a repository's archive is.
fn tarball(dir: &Path, top: &str, to: &Path) {
    let mut tar = Command::new("tar");
    tar.arg("-czf").arg(to).arg("-C").arg(dir).arg(top);
    run::within(tar, MOST, &|| {}).unwrap();
}

#[test]
fn an_archive_that_would_write_outside_its_folder_is_not_unpacked() {
    assert_eq!(
        inside("LLX Desk.app/\nLLX Desk.app/Contents/MacOS/llx-desk\n"),
        Ok(())
    );
    assert_eq!(inside("llx-desk\n./LICENSE\n"), Ok(()));
    // A name with two dots in it is a name; a step up is not.
    assert_eq!(inside("llx-desk..old\n"), Ok(()));
    for entry in [
        "../.zshrc",
        "LLX Desk.app/../../x",
        "/etc/hosts",
        "a/..",
        "..",
    ] {
        let why = inside(&format!("llx-desk\n{entry}\n")).unwrap_err();
        assert!(why.contains("outside"), "{entry}: {why}");
    }
}

#[test]
fn a_tarball_is_unpacked_without_its_top_folder() {
    let dir = folder("untar");
    std::fs::create_dir_all(dir.join("skills-main/skills/pdf")).unwrap();
    std::fs::write(dir.join("skills-main/skills/pdf/SKILL.md"), "---\n").unwrap();
    std::os::unix::fs::symlink("pdf/SKILL.md", dir.join("skills-main/skills/link")).unwrap();
    let archive = dir.join("main.tar.gz");
    tarball(&dir, "skills-main", &archive);
    let into = dir.join("into");
    untar(&archive, &into, 1, MOST).unwrap();
    assert_eq!(
        std::fs::read_to_string(into.join("skills/pdf/SKILL.md")).unwrap(),
        "---\n"
    );
    // A link that stays inside is kept.
    assert!(into.join("skills/link").is_symlink());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_tarball_with_a_link_out_of_its_folder_is_refused_and_leaves_nothing() {
    for (name, to) in [("absolute", "/etc/hosts"), ("up", "../../../.zshrc")] {
        let dir = folder(&format!("untar-{name}"));
        std::fs::create_dir_all(dir.join("repo-main/a")).unwrap();
        std::os::unix::fs::symlink(to, dir.join("repo-main/a/out")).unwrap();
        let archive = dir.join("main.tar.gz");
        tarball(&dir, "repo-main", &archive);
        let into = dir.join("into");
        let why = untar(&archive, &into, 1, MOST).unwrap_err();
        assert!(why.contains("`a/out` points outside"), "{name}: {why}");
        assert!(!into.exists(), "{name}");
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn a_path_is_inside_while_it_never_steps_up_past_its_start() {
    assert!(stays_inside(Path::new("a/b/../c")));
    assert!(stays_inside(Path::new("./a")));
    assert!(!stays_inside(Path::new("a/../../b")));
    assert!(!stays_inside(Path::new("/a")));
}
