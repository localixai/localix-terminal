//! Tests of `comments`.

use super::Comments;

#[test]
fn a_batch_is_one_message_per_tab_and_is_gone_once_taken() {
    let comments = Comments::default();
    comments.add("w1", "one".into());
    comments.add("w2", "two".into());
    comments.add("w1", "three".into());
    assert_eq!(comments.count(), 3);
    let sent = comments.take();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].0, "w1");
    assert!(sent[0].1.starts_with("The person sent 2 comment(s)"));
    assert!(sent[0].1.ends_with("one\n\nthree"));
    assert_eq!(comments.count(), 0);
    assert!(comments.take().is_empty());
}

#[test]
fn a_comments_picture_is_kept_in_the_homes_directory_for_the_person_alone() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = crate::testing::Scratch::new();
    let dir = scratch.path().join("comments");
    let path = crate::shots::save(&dir, b"jpeg").unwrap();
    let mode =
        |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&path), 0o600);
    assert!(super::directory().ends_with("comments"));
}
