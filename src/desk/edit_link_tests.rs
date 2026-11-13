//! Tests of `desk::edit_link`: which file a link of a document names.

use std::path::{Path, PathBuf};

use super::linked;

fn named(href: &str) -> Option<PathBuf> {
    linked(Path::new("/p/docs/guide.md"), Path::new("/p"), href)
}

#[test]
fn a_link_is_read_from_the_folder_of_the_file_that_holds_it() {
    assert_eq!(named("ui.md"), Some("/p/docs/ui.md".into()));
    assert_eq!(named("./ui.md"), Some("/p/docs/./ui.md".into()));
    assert_eq!(named("../README.md"), Some("/p/docs/../README.md".into()));
    assert_eq!(named("img/a%20b.png"), Some("/p/docs/img/a b.png".into()));
}

#[test]
fn a_link_from_the_root_is_read_from_the_projects_folder() {
    assert_eq!(named("/README.md"), Some("/p/README.md".into()));
}

#[test]
fn a_place_in_the_file_and_a_question_are_left_off() {
    assert_eq!(named("ui.md#colour"), Some("/p/docs/ui.md".into()));
    assert_eq!(named("ui.md?plain=1#colour"), Some("/p/docs/ui.md".into()));
}

#[test]
fn what_names_no_file_of_the_project_is_none() {
    assert_eq!(named("#colour"), None);
    assert_eq!(named(""), None);
    assert_eq!(named("https://example.com/a.md"), None);
    assert_eq!(named("mailto:a@example.com"), None);
    assert_eq!(named("file:///etc/passwd"), None);
    assert_eq!(named("//example.com/a.md"), None);
    assert_eq!(named("a%ffb.md"), None);
}
