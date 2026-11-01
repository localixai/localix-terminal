//! Tests of `web`.

use super::*;

#[test]
fn an_address_without_a_scheme_is_https_but_this_machine_is_http() {
    assert_eq!(
        web_address("example.com/a").unwrap(),
        "https://example.com/a"
    );
    assert_eq!(
        web_address("localhost:3000").unwrap(),
        "http://localhost:3000"
    );
    assert_eq!(
        web_address("127.0.0.1:80/x").unwrap(),
        "http://127.0.0.1:80/x"
    );
}

#[test]
fn only_the_web_opens_in_the_window() {
    assert!(
        web_address("file:///etc/passwd")
            .unwrap_err()
            .contains("only http")
    );
    assert!(web_address("javascript:alert(1)").is_err());
    assert!(web_address("http://a b").is_err());
}

#[test]
fn refs_are_dropped_from_the_outline() {
    assert_eq!(
        without_refs("- button \"Go\" [ref=e1]\n- link [ref=e2]: x"),
        "- button \"Go\"\n- link: x"
    );
}

#[test]
fn the_project_is_the_nearest_git_directory() {
    let scratch = crate::testing::Scratch::new();
    scratch.file("a/.git/HEAD", "");
    scratch.file("a/b/c/f", "");
    assert_eq!(
        project(&scratch.path().join("a/b/c")),
        scratch.path().join("a")
    );
    assert_eq!(project(scratch.path()), scratch.path());
}
