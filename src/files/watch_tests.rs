//! Tests of `watch`, against the real system: a change in a folder listened to is told as it
//! happens, one in a folder nobody listens to is not, and a watch dropped tells no more.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::Duration;

use super::{Change, Watch};
use crate::testing::Scratch;

/// Long enough for the system to tell of a change on a busy machine.
const SOON: Duration = Duration::from_secs(10);
/// Long enough that a change that would be told has been.
const QUIET: Duration = Duration::from_millis(600);

fn watched(dir: &Scratch, listened: &[&str]) -> (Watch, Receiver<Change>) {
    let (tell, told) = sync_channel(64);
    let watch = Watch::new(dir.path(), move |change| {
        let _ = tell.try_send(change);
    })
    .unwrap();
    let dirs: BTreeSet<PathBuf> = (listened.iter()).map(|rel| dir.path().join(rel)).collect();
    watch.listen(&dirs);
    (watch, told)
}

/// The folders told of until `dir` is among them.
fn told_of(told: &Receiver<Change>, dir: &std::path::Path) -> bool {
    let deadline = std::time::Instant::now() + SOON;
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        match told.recv_timeout(left) {
            Ok(Change::Dirs(dirs)) if dirs.iter().any(|told| told == dir) => return true,
            Ok(Change::All) => return true,
            Ok(Change::Dirs(_)) => {}
            Err(_) => return false,
        }
    }
    false
}

#[test]
fn a_file_made_removed_or_renamed_in_a_folder_listened_to_is_told() {
    let dir = Scratch::new();
    dir.file("src/main.rs", "");
    let (_watch, told) = watched(&dir, &["", "src"]);
    dir.file("src/new.rs", "");
    assert!(told_of(&told, &dir.path().join("src")), "a file made");
    std::fs::rename(
        dir.path().join("src/new.rs"),
        dir.path().join("src/renamed.rs"),
    )
    .unwrap();
    assert!(told_of(&told, &dir.path().join("src")), "a file renamed");
    std::fs::remove_file(dir.path().join("src/renamed.rs")).unwrap();
    assert!(told_of(&told, &dir.path().join("src")), "a file removed");
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    assert!(told_of(&told, dir.path()), "a folder made in the root");
}

#[test]
fn a_change_in_a_folder_nobody_listens_to_wakes_no_one() {
    let dir = Scratch::new();
    dir.file("target/debug/a.o", "");
    dir.file("src/main.rs", "");
    let (watch, told) = watched(&dir, &["", "src"]);
    for at in 0..50 {
        dir.file(&format!("target/debug/{at}.o"), "");
    }
    if let Ok(change) = told.recv_timeout(QUIET) {
        panic!("nothing listened to changed, yet {change:?} was told");
    }
    // Listened to from now on, it is heard.
    let dirs: BTreeSet<PathBuf> = [dir.path().join("target/debug")].into();
    watch.listen(&dirs);
    dir.file("target/debug/late.o", "");
    assert!(told_of(&told, &dir.path().join("target/debug")));
}

#[test]
fn a_watch_dropped_tells_no_more() {
    let dir = Scratch::new();
    let (watch, told) = watched(&dir, &[""]);
    drop(watch);
    dir.file("after.txt", "");
    assert!(told.recv_timeout(QUIET).is_err());
}
