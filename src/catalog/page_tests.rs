//! Tests of the Store's page.

use super::*;

#[test]
fn the_page_runs_its_one_script_by_its_hash_and_its_title_is_text() {
    let page = page("desk · store · <b>shop</b>");
    let digest = crate::page::digest(SCRIPT);
    assert_eq!(
        SCRIPT_SHA384, digest,
        "page.js changed: put its new SHA-384 here"
    );
    assert!(page.contains(&format!("script-src 'sha384-{SCRIPT_SHA384}'")));
    assert!(page.contains("default-src 'none'"));
    assert!(page.contains("<title>desk · store · &lt;b&gt;shop&lt;/b&gt;</title>"));
    assert!(!page.contains("{{"), "every place is filled");
    assert!(!SCRIPT.to_ascii_lowercase().contains("</script"));
    assert!(
        !SCRIPT.contains("innerHTML"),
        "what a listing says is set as text"
    );
    assert!(
        !SCRIPT.contains("href"),
        "an address is opened by the app, never by the page"
    );
}

#[test]
fn a_frame_carries_what_a_publisher_wrote_as_data() {
    let told = frame("found", &serde_json::json!({"t": "</script>\u{2028}\"x\""}));
    assert!(told.starts_with("S(\"found\","));
    assert!(!told.contains('\u{2028}'));
    let data = told
        .strip_prefix("S(\"found\",")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let back: Value = serde_json::from_str(data).unwrap();
    assert_eq!(back["t"], "</script>\u{2028}\"x\"");
}
