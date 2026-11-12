//! Tests of `paths`.

use super::*;

#[test]
fn the_runtime_directory_is_named_for_the_user_alone() {
    assert_eq!(
        under(Path::new("/var/folders/x/T"), 501),
        Path::new("/var/folders/x/T/llx-desk-501")
    );
}

#[test]
fn the_runtime_directory_is_made_private_and_a_link_in_its_place_is_refused() {
    let scratch = crate::testing::Scratch::new();
    let made = scratch.path().join("made");
    own_private_dir(&made).unwrap();
    assert_eq!(std::fs::metadata(&made).unwrap().mode() & 0o777, 0o700);

    // Someone put a link at the predictable name: it is not used.
    let link = scratch.path().join("link");
    std::os::unix::fs::symlink(&made, &link).unwrap();
    assert!(own_private_dir(&link).is_err());
}

#[test]
fn an_app_given_a_runtime_directory_is_apart_from_the_persons() {
    let temp = Path::new("/var/folders/xy/T");
    assert_eq!(
        runtime_from(Some("/tmp/apart".into()), temp, 501),
        Path::new("/tmp/apart")
    );
    // Named empty, or not at all, it is the person's own.
    assert_eq!(runtime_from(Some("".into()), temp, 501), under(temp, 501));
    assert_eq!(runtime_from(None, temp, 501), under(temp, 501));
    // The same naming says whether the app is apart, and so leaves the person's setup alone.
    assert_eq!(named(Some("/tmp/apart".into())), Some("/tmp/apart".into()));
    assert_eq!(named(Some("".into())), None);
    assert_eq!(named(None), None);
}
