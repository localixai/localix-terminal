//! Tests of `webview`.

use super::*;

#[test]
fn another_fragment_of_the_same_page_is_no_navigation() {
    assert!(fragment_only("https://app/", "https://app/#/settings"));
    assert!(fragment_only("https://app/#a", "https://app/#b"));
    assert!(fragment_only("https://app/?q=1#a", "https://app/?q=1#"));
}

#[test]
fn anything_else_is_a_navigation() {
    // No fragment in the target, even where the current one has one: the page loads again.
    assert!(!fragment_only("https://app/#a", "https://app/"));
    assert!(!fragment_only("https://app/", "https://app/other#a"));
    assert!(!fragment_only("https://app/?q=1", "https://app/?q=2#a"));
    assert!(!fragment_only("https://app/", "https://other/#a"));
}
