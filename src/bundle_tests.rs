//! Tests of `bundle`: which program is an app's, and which bundle is a temporary one.

use super::{adds_only_options, of, temporary};
use std::path::Path;

#[test]
fn the_program_of_a_bundle_names_it_and_any_other_names_none() {
    let exe = Path::new("/Applications/Localix Terminal.app/Contents/MacOS/llxterm");
    assert_eq!(
        of(exe),
        Some(Path::new("/Applications/Localix Terminal.app"))
    );
    assert_eq!(of(Path::new("/usr/local/bin/llxterm")), None);
    assert_eq!(of(Path::new("/x/Contents/MacOS/llxterm")), None);
    assert_eq!(of(Path::new("llxterm")), None);
}

#[test]
fn a_run_is_the_app_for_what_the_system_adds_and_the_command_for_the_rest() {
    let args = |words: &[&str]| words.iter().map(|w| (*w).to_string()).collect::<Vec<_>>();
    assert!(adds_only_options(&args(&[])));
    assert!(adds_only_options(&args(&["-psn_0_12345"])));
    assert!(adds_only_options(&args(&[
        "-ApplePersistenceIgnoreState",
        "YES"
    ])));
    assert!(!adds_only_options(&args(&["--help"])));
    assert!(!adds_only_options(&args(&["-V"])));
    assert!(!adds_only_options(&args(&["window", "{}"])));
}

#[test]
fn an_app_on_a_disk_image_or_translocated_is_not_in_place() {
    assert!(temporary(Path::new(
        "/Volumes/Localix Terminal/Localix Terminal.app"
    )));
    assert!(temporary(Path::new(
        "/private/var/folders/x/AppTranslocation/ABC/d/Localix Terminal.app"
    )));
    assert!(!temporary(Path::new("/Applications/Localix Terminal.app")));
    assert!(!temporary(Path::new(
        "/Users/me/Applications/Localix Terminal.app"
    )));
}

#[test]
fn a_bundle_under_llx_desks_name_takes_this_apps_where_nothing_has_it() {
    let scratch = crate::testing::Scratch::new();
    let old = scratch.path().join("LLX Desk.app");
    std::fs::create_dir_all(old.join("Contents/MacOS")).unwrap();
    assert_eq!(
        super::renamed(&old),
        Some(scratch.path().join("Localix Terminal.app"))
    );
    // One that is there already is the person's: neither is moved.
    std::fs::create_dir_all(scratch.path().join("Localix Terminal.app")).unwrap();
    assert_eq!(super::renamed(&old), None);
    // Any other name is the person's choice, or this app's already.
    for name in ["Localix Terminal.app", "LLX Desk 2.app", "Mine.app"] {
        assert_eq!(super::renamed(&scratch.path().join(name)), None, "{name}");
    }
}
