//! Tests of `command`.

use super::*;
use crate::testing::Scratch;

#[test]
fn the_command_is_linked_once_and_another_program_is_left_alone() {
    let scratch = Scratch::new();
    let exe = scratch.path().join("LLX Desk.app/Contents/MacOS/llx-desk");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::write(&exe, "").unwrap();
    let at = scratch.path().join(".local/bin/llx-desk");
    assert!(link(&exe, &at).unwrap().contains("links to"));
    assert_eq!(std::fs::read_link(&at).unwrap(), exe);
    assert!(link(&exe, &at).unwrap().contains("linked already"));
    // A link to another app's program is moved to this one.
    let other = scratch.path().join("old");
    std::fs::remove_file(&at).unwrap();
    std::os::unix::fs::symlink(&other, &at).unwrap();
    link(&exe, &at).unwrap();
    assert_eq!(std::fs::read_link(&at).unwrap(), exe);
    // A file that is no llx-desk stays.
    std::fs::remove_file(&at).unwrap();
    std::fs::write(&at, "#!/bin/sh\necho mine\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
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
        text.starts_with("eval \"$(brew shellenv)\"\n\n# Added by LLX Desk"),
        "{text}"
    );
    assert_eq!(text.matches("export PATH").count(), 1);
}
