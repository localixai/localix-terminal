//! Tests of `command`.

use super::*;
use crate::testing::Scratch;
use std::os::unix::fs::PermissionsExt as _;

#[test]
fn the_command_is_linked_once_and_another_program_is_left_alone() {
    let scratch = Scratch::new();
    let exe = scratch
        .path()
        .join("Localix Terminal.app/Contents/MacOS/llxterm");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::write(&exe, "").unwrap();
    let at = scratch.path().join(".local/bin/llxterm");
    assert!(link(&exe, &at).unwrap().contains("links to"));
    assert_eq!(std::fs::read_link(&at).unwrap(), exe);
    assert!(link(&exe, &at).unwrap().contains("linked already"));
    // A link to another app's program is moved to this one.
    let other = scratch.path().join("old");
    std::fs::remove_file(&at).unwrap();
    std::os::unix::fs::symlink(&other, &at).unwrap();
    link(&exe, &at).unwrap();
    assert_eq!(std::fs::read_link(&at).unwrap(), exe);
    // A link to a program of the person's own stays, one to nothing is replaced.
    let mine = scratch.path().join("mine");
    std::fs::write(&mine, "#!/bin/sh\necho mine\n").unwrap();
    std::fs::set_permissions(&mine, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::remove_file(&at).unwrap();
    std::os::unix::fs::symlink(&mine, &at).unwrap();
    assert!(link(&exe, &at).is_err());
    assert_eq!(std::fs::read_link(&at).unwrap(), mine);
    // A file that is no llxterm stays.
    std::fs::remove_file(&at).unwrap();
    std::fs::write(&at, "#!/bin/sh\necho mine\n").unwrap();
    std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(link(&exe, &at).is_err());
    assert_eq!(
        std::fs::read_to_string(&at).unwrap(),
        "#!/bin/sh\necho mine\n"
    );
}

#[test]
fn the_profile_gets_the_path_once() {
    let scratch = Scratch::new();
    let profile = profile_of("/bin/zsh", scratch.path());
    assert!(profile.ends_with(".zprofile"));
    assert!(profile_of("/usr/local/bin/fish", scratch.path()).ends_with(".profile"));
    std::fs::write(&profile, "eval \"$(brew shellenv)\"").unwrap();
    assert!(add_to_profile(&profile).unwrap());
    assert!(!add_to_profile(&profile).unwrap());
    let text = std::fs::read_to_string(&profile).unwrap();
    assert!(
        text.starts_with("eval \"$(brew shellenv)\"\n\n# Added by Localix Terminal"),
        "{text}"
    );
    assert_eq!(text.matches("export PATH").count(), 1);
}

#[test]
fn a_bash_login_shell_keeps_the_profile_it_reads() {
    let scratch = Scratch::new();
    let home = scratch.path();
    assert!(profile_of("/bin/bash", home).ends_with(".bash_profile"));
    // Bash reads only the first of these: a new `.bash_profile` would hide `.profile`.
    std::fs::write(home.join(".profile"), "").unwrap();
    assert!(profile_of("/bin/bash", home).ends_with(".profile"));
    std::fs::write(home.join(".bash_login"), "").unwrap();
    assert!(profile_of("/bin/bash", home).ends_with(".bash_login"));
    std::fs::write(home.join(".bash_profile"), "").unwrap();
    assert!(profile_of("/bin/bash", home).ends_with(".bash_profile"));
}

#[test]
fn the_command_is_taken_off_as_state_found_it_and_a_file_is_never_removed() {
    let scratch = Scratch::new();
    let exe = scratch
        .path()
        .join("Localix Terminal.app/Contents/MacOS/llxterm");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::write(&exe, "").unwrap();
    let exe = std::fs::canonicalize(&exe).unwrap();
    let home = scratch.path().join("home");
    let at = link_path(&home);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    // A link to a link to the program: `state` finds it, and so does taking it off.
    let middle = scratch.path().join("usr-local-llxterm");
    std::os::unix::fs::symlink(&exe, &middle).unwrap();
    std::os::unix::fs::symlink(&middle, &at).unwrap();
    assert!(take_off_path(&exe, &home).unwrap().contains("is gone"));
    assert!(std::fs::symlink_metadata(&at).is_err());
    // A copy of the program is no link of this app's.
    std::fs::write(&at, "").unwrap();
    assert!(take_off_path(&exe, &home).unwrap().contains("was not"));
    assert!(at.exists());
}
