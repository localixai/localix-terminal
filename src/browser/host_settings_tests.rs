//! Tests of `settings`, the window's Settings page.

use sha2::Digest as _;

use super::*;

#[test]
fn the_page_runs_its_one_script_by_its_hash() {
    let page = page();
    let script = page
        .split("<script>")
        .nth(1)
        .and_then(|s| s.split("</script>").next());
    let digest = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        sha2::Sha384::digest(script.unwrap_or_default()),
    );
    assert_eq!(
        SCRIPT_SHA384, digest,
        "settings.html's script changed: put its new SHA-384 here"
    );
    assert_eq!(page.matches("<script").count(), 1, "one script runs");
    assert!(page.contains(&format!("script-src 'sha384-{SCRIPT_SHA384}'\"")));
    let policy = page
        .split("script-src")
        .nth(1)
        .and_then(|s| s.split('"').next());
    assert!(!policy.unwrap_or_default().contains("unsafe"), "{policy:?}");
    assert!(!page.contains("{{"), "every place is filled");
}
