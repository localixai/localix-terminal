//! Tests of `drafts`: what is unsaved is kept by its file's path, replaced as the person goes
//! on, forgotten once saved, knows whether the disk changed under it, and is one of a bounded few.

use std::path::Path;

use super::*;
use crate::files::text;
use crate::store::Store;
use crate::testing::Scratch;

#[test]
fn what_is_unsaved_is_kept_by_its_path_until_it_is_forgotten() {
    let dir = Scratch::new();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, "one\n").unwrap();
    let held = text::read(&file).unwrap();
    let store = Store::memory().unwrap();
    assert_eq!(store.read(|db| draft(db, &file)).unwrap(), None);

    store
        .write(|tx| keep(tx, &file, "one\ntwo", Some(&held.stamp)))
        .unwrap();
    store
        .write(|tx| keep(tx, &file, "one\ntwo\nthree", Some(&held.stamp)))
        .unwrap();
    let kept = store.read(|db| draft(db, &file)).unwrap().unwrap();
    assert_eq!(kept.text, "one\ntwo\nthree");
    assert!(!kept.moved(&held.stamp));
    assert_eq!(
        store.read(|db| draft(db, Path::new("/no/such"))).unwrap(),
        None
    );

    // Another hand writes the file: the draft knows the disk moved under it.
    std::fs::write(&file, "another\n").unwrap();
    assert!(kept.moved(&text::read(&file).unwrap().stamp));

    store.write(|tx| forget(tx, &file)).unwrap();
    assert_eq!(store.read(|db| draft(db, &file)).unwrap(), None);
}

#[test]
fn past_the_most_files_the_ones_written_longest_ago_go() {
    let store = Store::memory().unwrap();
    let file = |n: u32| std::path::PathBuf::from(format!("/work/{n}.txt"));
    let kept = |n: u32| store.read(|db| draft(db, &file(n))).unwrap().is_some();
    for n in 0..MOST {
        store.write(|tx| keep(tx, &file(n), "mine", None)).unwrap();
    }
    assert!(kept(0) && kept(MOST - 1));

    // The first is written again: it is the newest, and the next one in line goes instead.
    store.write(|tx| keep(tx, &file(0), "more", None)).unwrap();
    store
        .write(|tx| keep(tx, &file(MOST), "mine", None))
        .unwrap();
    assert!(kept(0) && kept(MOST) && kept(2));
    assert!(!kept(1));
}
