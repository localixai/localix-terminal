//! Tests of `former`: LLX Desk's command link and desktop entry taken away only when they are
//! its own and run nothing now, or run this app.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::{Swept, desktop, link};
use crate::testing::Scratch;

/// This app's program, in a bundle under the scratch directory.
fn this_program(scratch: &Scratch) -> PathBuf {
    let exe = scratch.file("Localix Terminal.app/Contents/MacOS/llxterm", "");
    std::fs::canonicalize(exe).unwrap()
}

fn linked(home: &Path, to: &Path) -> PathBuf {
    let at = home.join(".local/bin/llx-desk");
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&at);
    std::os::unix::fs::symlink(to, &at).unwrap();
    at
}

fn script(at: &Path, says: &str) {
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, format!("#!/bin/sh\necho '{says}'\n")).unwrap();
    std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn llx_desks_link_goes_once_its_program_is_gone_or_it_runs_this_app() {
    let scratch = Scratch::new();
    let exe = this_program(&scratch);
    let home = scratch.path().join("home");
    assert_eq!(link(&exe, &home), Swept::Nothing);
    // As 0.1.6 made it, to the program in its bundle, which the update replaced.
    let gone = scratch.path().join("LLX Desk.app/Contents/MacOS/llx-desk");
    let at = linked(&home, &gone);
    assert!(matches!(link(&exe, &home), Swept::Done(_)));
    assert!(std::fs::symlink_metadata(&at).is_err());
    // One to this program, or to another llxterm, is this app's under the old name.
    linked(&home, &exe);
    assert!(matches!(link(&exe, &home), Swept::Done(_)));
    let other = scratch.path().join("elsewhere/llxterm");
    script(&other, "llxterm 0.1.7");
    linked(&home, &other);
    assert!(matches!(link(&exe, &home), Swept::Done(_)));
}

#[test]
fn a_link_that_runs_another_program_and_a_file_stay() {
    let scratch = Scratch::new();
    let exe = this_program(&scratch);
    let home = scratch.path().join("home");
    // An LLX Desk installed beside this app still runs: it is the person's.
    let kept = scratch.path().join("cargo/bin/llx-desk");
    script(&kept, "llx-desk 0.1.6");
    let at = linked(&home, &kept);
    assert!(matches!(link(&exe, &home), Swept::Left(_)));
    assert!(std::fs::symlink_metadata(&at).is_ok());
    // A link to nothing of another name is not LLX Desk's.
    linked(&home, &scratch.path().join("mine"));
    assert!(matches!(link(&exe, &home), Swept::Left(_)));
    // A file is never removed, whatever it is.
    std::fs::remove_file(&at).unwrap();
    script(&at, "llxterm 0.1.7");
    assert!(matches!(link(&exe, &home), Swept::Left(_)));
    assert!(at.is_file());
}

#[test]
fn llx_desks_desktop_entry_goes_once_the_program_it_opens_is_gone() {
    let scratch = Scratch::new();
    let exe = this_program(&scratch);
    let home = scratch.path().join("home");
    assert_eq!(desktop(&exe, &home), Swept::Nothing);
    let entry = |program: &Path, name: &str| {
        let file = home.join(".local/share/applications/llx-desk.desktop");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        // As 0.1.6 wrote it.
        let text = format!(
            "[Desktop Entry]\nType=Application\nName={name}\nComment=Pages and terminals agents work in, and you watch\nExec=\"{}\" open\nTerminal=false\nCategories=Development;\n",
            program.display()
        );
        std::fs::write(&file, text).unwrap();
        file
    };
    let file = entry(&home.join(".local/share/llx-desk/llx-desk"), "LLX Desk");
    assert!(matches!(desktop(&exe, &home), Swept::Done(_)));
    assert!(!file.exists());
    // One that opens a program that is there, or that is not LLX Desk's, stays.
    let kept = scratch.file("share/llx-desk/llx-desk", "");
    entry(&kept, "LLX Desk");
    assert!(matches!(desktop(&exe, &home), Swept::Left(_)));
    let file = entry(&home.join("gone"), "Something Else");
    assert!(matches!(desktop(&exe, &home), Swept::Left(_)));
    assert!(file.exists());
    // One that opens this app is this app's under the old name.
    entry(&exe, "LLX Desk");
    assert!(matches!(desktop(&exe, &home), Swept::Done(_)));
}
