//! Tests of `former`: LLX Desk 0.1.6's folders, database and program carried to this app's
//! names, in a scratch directory, never the person's home.

use std::os::unix::io::AsRawFd as _;
use std::path::Path;

use super::{database, folder, held_by, program};
use crate::testing::Scratch;

fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn the_data_folder_is_renamed_whole_with_its_database_and_its_journal() {
    let scratch = Scratch::new();
    let share = scratch.path().join(".local/share");
    scratch.file(".local/share/llx-desk/llx-desk.sqlite", "db");
    scratch.file(".local/share/llx-desk/llx-desk.sqlite-wal", "wal");
    scratch.file(".local/share/llx-desk/llx-desk.sqlite-shm", "shm");
    scratch.file(".local/share/llx-desk/shell/zsh/.zshrc", "rc");
    let said = folder(&share.join("llx-desk"), &share.join("llxterm"), true);
    assert_eq!(said.len(), 1, "{said:?}");
    let now = format!("is now {}", share.join("llxterm").display());
    assert!(said[0].ends_with(&now), "{said:?}");
    assert!(!share.join("llx-desk").exists());
    assert_eq!(text(&share.join("llxterm/llxterm.sqlite")), "db");
    assert_eq!(text(&share.join("llxterm/llxterm.sqlite-wal")), "wal");
    assert_eq!(text(&share.join("llxterm/llxterm.sqlite-shm")), "shm");
    assert_eq!(text(&share.join("llxterm/shell/zsh/.zshrc")), "rc");
    // Done once: there is nothing left to carry the second time.
    assert!(folder(&share.join("llx-desk"), &share.join("llxterm"), true).is_empty());
}

#[test]
fn a_database_that_cannot_move_whole_leaves_the_folder_as_it_was() {
    let scratch = Scratch::new();
    let old = scratch.path().join("llx-desk");
    let new = scratch.path().join("llxterm");
    scratch.file("llx-desk/llx-desk.sqlite", "db");
    scratch.file("llx-desk/llx-desk.sqlite-wal", "wal");
    // A journal under the new name already, with no database: the two would not belong together.
    scratch.file("llx-desk/llxterm.sqlite-wal", "stray");
    let said = folder(&old, &new, true);
    assert!(said[0].contains("stays"), "{said:?}");
    assert!(!new.exists());
    assert_eq!(text(&old.join("llx-desk.sqlite")), "db");
    assert_eq!(text(&old.join("llx-desk.sqlite-wal")), "wal");
}

#[test]
fn a_folder_this_app_made_without_a_database_is_given_what_it_lacks() {
    let scratch = Scratch::new();
    let old = scratch.path().join("llx-desk");
    let new = scratch.path().join("llxterm");
    scratch.file("llx-desk/llx-desk.sqlite", "db");
    scratch.file("llx-desk/window.json", "old window");
    scratch.file("llx-desk/catalog/index", "index");
    // install.sh's program, as 0.1.6 and 0.1.7 put it on Linux: neither is data.
    scratch.file("llx-desk/llx-desk", "old program");
    scratch.file("llxterm/llxterm", "new program");
    scratch.file("llxterm/window.json", "new window");
    let said = folder(&old, &new, true);
    assert_eq!(text(&new.join("llxterm.sqlite")), "db");
    assert_eq!(text(&new.join("catalog/index")), "index");
    // What the new folder has is its own; the old program stays with what stayed.
    assert_eq!(text(&new.join("window.json")), "new window");
    assert_eq!(text(&old.join("window.json")), "old window");
    assert_eq!(text(&old.join("llx-desk")), "old program");
    assert!(!new.join("llx-desk").exists());
    assert!(
        said.iter().any(|line| line.contains("window.json stays")),
        "{said:?}"
    );
}

#[test]
fn a_folder_this_app_has_used_already_is_not_mixed_with_the_old() {
    let scratch = Scratch::new();
    let old = scratch.path().join("llx-desk");
    let new = scratch.path().join("llxterm");
    scratch.file("llx-desk/llx-desk.sqlite", "old");
    scratch.file("llx-desk/catalog/index", "index");
    scratch.file("llxterm/llxterm.sqlite", "new");
    let said = folder(&old, &new, true);
    assert!(said[0].contains("has a database of its own"), "{said:?}");
    assert_eq!(text(&old.join("llx-desk.sqlite")), "old");
    assert!(old.join("catalog/index").exists() && !new.join("catalog").exists());
    // Settings have no database: the new folder is given what it lacks.
    scratch.file("config/llx-desk/secrets/token", "secret");
    scratch.file("config/llxterm/settings.json", "{}");
    let config = scratch.path().join("config");
    folder(&config.join("llx-desk"), &config.join("llxterm"), false);
    assert_eq!(text(&config.join("llxterm/secrets/token")), "secret");
    assert!(
        !config.join("llx-desk").exists(),
        "an emptied folder is removed"
    );
}

#[test]
fn a_folder_the_person_named_keeps_its_place_and_its_database_takes_the_new_name() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    scratch.file("llx-desk.sqlite", "db");
    scratch.file("llx-desk.sqlite-wal", "wal");
    assert_eq!(database(dir).len(), 1);
    assert_eq!(text(&dir.join("llxterm.sqlite")), "db");
    assert_eq!(text(&dir.join("llxterm.sqlite-wal")), "wal");
    assert!(!dir.join("llx-desk.sqlite").exists());
    // Once this app has its own, an old one found beside it is left alone, and nothing is said.
    scratch.file("llx-desk.sqlite", "older");
    assert!(database(dir).is_empty());
    assert_eq!(text(&dir.join("llxterm.sqlite")), "db");
    assert_eq!(text(&dir.join("llx-desk.sqlite")), "older");
    // No folder of 0.1.6's: nothing to do.
    assert!(folder(&dir.join("absent"), &dir.join("llxterm"), true).is_empty());
}

#[test]
fn only_the_program_that_runs_takes_the_place_install_sh_gives_this_app() {
    let scratch = Scratch::new();
    let share = scratch.path();
    // 0.1.6's updater put this app at its own program's place; the data folder came along.
    let exe = scratch.file("llxterm/llx-desk", "this program");
    let other = scratch.file("elsewhere/llxterm", "another program");
    assert!(program(share, &other).is_empty());
    assert!(exe.exists());
    let said = program(share, &exe);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(text(&share.join("llxterm/llxterm")), "this program");
    assert!(!exe.exists());
    // Where the program kept a folder of its own (XDG_DATA_HOME apart from the data).
    let scratch = Scratch::new();
    let share = scratch.path();
    let exe = scratch.file("llx-desk/llx-desk", "this program");
    program(share, &exe);
    assert_eq!(text(&share.join("llxterm/llxterm")), "this program");
    assert!(!share.join("llx-desk").exists());
    // A program there already is never replaced.
    let exe = scratch.file("llx-desk/llx-desk", "again");
    assert!(program(share, &exe).is_empty());
    assert_eq!(text(&share.join("llxterm/llxterm")), "this program");
}

#[test]
fn llx_desk_is_known_to_run_while_it_holds_its_lock() {
    let scratch = Scratch::new();
    let lock = scratch.file("llx-desk-501/desk.lock", "4242");
    assert_eq!(held_by(&lock), None);
    let holder = std::fs::File::open(&lock).unwrap();
    // SAFETY: flock on a descriptor this test owns.
    let taken = unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    assert_eq!(taken, 0);
    assert_eq!(held_by(&lock), Some(4242));
    drop(holder);
    assert_eq!(held_by(&lock), None);
    assert_eq!(held_by(&scratch.path().join("absent")), None);
}
