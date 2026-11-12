//! Tests of the window's small pictures: each is one stroked shape, the same in a menu and on
//! the strip.

use super::super::strip::PAGE;
use super::*;

/// What each path of `picture` draws: its `d`.
fn drawn(picture: &str) -> Vec<&str> {
    (picture.split(" d=\"").skip(1))
        .filter_map(|rest| rest.split('"').next())
        .collect()
}

#[test]
fn a_picture_is_one_stroked_shape_the_same_in_a_menu_and_on_the_strip() {
    let kinds = ["web", "term", "board", "file", "settings"].map(|kind| (kind, of_kind(kind)));
    for (name, picture) in kinds.into_iter().chain([("folder", FOLDER)]) {
        // The ink is not opaque, so two shapes that cross are painted twice where they do, a
        // brighter spot at every joint. Lines that meet are one path, which is painted once.
        let paths = drawn(picture);
        assert_eq!(paths.len(), 1, "{name}");
        assert!(!picture.contains("<rect"), "{name}");
        // The strip draws the same picture under the same name.
        let strips = format!("{name}: [[\"path\", {{ d: \"{}\" }}]", paths[0]);
        assert!(PAGE.contains(&strips), "{name}");
    }
}
