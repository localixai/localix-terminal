//! Tests of `files`: a folder read in the order a person expects, what the tree never shows,
//! a long folder counted past what is listed, a tree's paths held under the root, and the open
//! folders kept per project.

use std::path::Path;

use super::{Entry, MOST, list, open, open_dirs, shut, within};
use crate::store::Store;
use crate::testing::Scratch;

fn names(dir: &Path) -> Vec<String> {
    (list(dir).unwrap().entries.into_iter())
        .map(|entry| entry.name)
        .collect()
}

#[test]
fn a_folder_lists_folders_first_then_names_as_a_file_manager_sorts_them() {
    let dir = Scratch::new();
    for file in [
        "file10.txt",
        "file2.txt",
        "Zebra.md",
        "apple.md",
        "File1.txt",
    ] {
        dir.file(file, "");
    }
    dir.file("src/main.rs", "");
    dir.file("Docs/readme.md", "");
    assert_eq!(
        names(dir.path()),
        [
            "Docs",
            "src",
            "apple.md",
            "File1.txt",
            "file2.txt",
            "file10.txt",
            "Zebra.md"
        ]
    );
    let listed = list(dir.path()).unwrap();
    assert_eq!(
        listed.entries.first(),
        Some(&Entry {
            name: "Docs".into(),
            dir: true
        })
    );
    assert_eq!(listed.more, 0);
}

#[test]
fn the_repository_store_and_the_finders_file_are_not_listed_and_dot_files_are() {
    let dir = Scratch::new();
    dir.file(".git/HEAD", "");
    dir.file(".DS_Store", "");
    dir.file(".gitignore", "");
    dir.file(".github/ci.yml", "");
    assert_eq!(names(dir.path()), [".github", ".gitignore"]);
}

#[test]
fn a_link_to_a_folder_is_a_folder_and_a_broken_link_a_file() {
    let dir = Scratch::new();
    dir.file("real/inside.txt", "");
    std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("linked")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("nowhere"), dir.path().join("broken")).unwrap();
    let listed = list(dir.path()).unwrap().entries;
    let kinds: Vec<(&str, bool)> = (listed.iter())
        .map(|entry| (entry.name.as_str(), entry.dir))
        .collect();
    assert_eq!(kinds, [("linked", true), ("real", true), ("broken", false)]);
}

#[test]
fn a_folder_longer_than_the_tree_lists_says_how_many_more_it_holds() {
    let dir = Scratch::new();
    for at in 0..MOST + 3 {
        dir.file(&format!("f{at}"), "");
    }
    let listed = list(dir.path()).unwrap();
    assert_eq!(listed.entries.len(), MOST);
    assert_eq!(listed.more, 3);
    assert_eq!(listed.entries.first().unwrap().name, "f0");
}

#[test]
fn a_folder_that_is_not_there_is_an_error_not_an_empty_one() {
    let dir = Scratch::new();
    assert!(list(&dir.path().join("gone")).is_err());
}

#[test]
fn a_trees_path_names_a_folder_under_the_root_or_nothing() {
    let root = Path::new("/work/app");
    assert_eq!(within(root, ""), Some(root.to_path_buf()));
    assert_eq!(within(root, "src/ui"), Some(root.join("src/ui")));
    assert_eq!(within(root, "a b/c"), Some(root.join("a b/c")));
    for outside in [
        "..",
        "src/../..",
        "/etc",
        "src//ui",
        "src/",
        "./src",
        "src/.",
        "a\0b",
    ] {
        assert_eq!(within(root, outside), None, "{outside:?}");
    }
}

#[test]
fn open_folders_are_kept_per_project_and_shutting_one_shuts_what_is_under_it() {
    let store = Store::memory().unwrap();
    let (one, two) = store
        .write(|tx| {
            let one = crate::projects::find_or_make(tx, Path::new("/work/one"), 1)?.id;
            let two = crate::projects::find_or_make(tx, Path::new("/work/two"), 1)?.id;
            for path in ["src", "src/ui", "src_old", "docs"] {
                assert!(open(tx, one, path)?);
            }
            assert!(!open(tx, one, "src")?, "open already");
            open(tx, two, "src")?;
            Ok((one, two))
        })
        .unwrap();
    let kept = |project| store.read(|db| open_dirs(db, project)).unwrap();
    assert_eq!(kept(one), ["docs", "src", "src/ui", "src_old"]);
    let shut_any = store.write(|tx| shut(tx, one, "src")).unwrap();
    assert!(shut_any);
    assert_eq!(
        kept(one),
        ["docs", "src_old"],
        "a name that only starts the same stays"
    );
    assert_eq!(kept(two), ["src"], "another project's are its own");
    assert!(!store.write(|tx| shut(tx, one, "src")).unwrap());
}
