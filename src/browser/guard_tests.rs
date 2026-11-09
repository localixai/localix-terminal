//! Tests of `guard`.

use super::*;

#[test]
fn an_agent_has_a_local_address_and_is_told_why_not_another() {
    assert_eq!(
        for_agent("http://localhost:3000/x").as_deref(),
        Ok("http://localhost:3000/x")
    );
    assert_eq!(for_agent("about:blank").as_deref(), Ok("about:blank"));
    let refused = for_agent("https://example.com/a").unwrap_err();
    assert_eq!(refused, not_local("https://example.com/a"));
    assert!(refused.contains("only local addresses"), "{refused}");
    assert!(
        refused.ends_with("external: https://example.com/a"),
        "{refused}"
    );
    assert!(
        for_agent("http://localhost@evil.com/")
            .unwrap_err()
            .contains("external")
    );
    assert!(
        for_agent("https:// example.com")
            .unwrap_err()
            .contains("not an address")
    );
}

#[test]
fn a_frame_loads_from_anywhere_on_the_web_but_never_a_file() {
    assert!(allowed("https://example.com/embed"));
    assert!(allowed("about:blank"));
    assert!(!allowed("file:///etc/passwd"));
    assert!(!allowed("data:text/html,hi"));
}

#[test]
fn a_page_sends_the_persons_browser_one_address_a_second_per_tab() {
    let mut throttle = Throttle::default();
    let start = Instant::now();
    assert!(throttle.let_through(1, start));
    assert!(!throttle.let_through(1, start + Duration::from_millis(300)));
    assert!(!throttle.let_through(1, start + Duration::from_millis(999)));
    // Another tab has a second of its own.
    assert!(throttle.let_through(2, start + Duration::from_millis(300)));
    assert!(throttle.let_through(1, start + Duration::from_millis(1000)));
    assert!(!throttle.let_through(1, start + Duration::from_millis(1500)));
}
