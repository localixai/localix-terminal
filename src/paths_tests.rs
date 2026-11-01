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
