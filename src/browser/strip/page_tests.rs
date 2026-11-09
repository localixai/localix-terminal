//! The strip's page as it is built: its one script run by its hash, the agents in it as data.

use sha2::Digest as _;

use super::*;

#[test]
fn the_assembled_page_contains_its_resize_helper_and_no_external_script() {
    let page = page("{}", "[]");
    assert!(page.contains("function resizeHandle("));
    assert!(!page.contains("/*RESIZE_HANDLES*/"));
    assert!(!page.contains("<script src="));
}

#[test]
fn the_page_runs_its_one_script_by_its_hash() {
    let page = page("{}", "[]");
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
        "strip.html's script changed: put its new SHA-384 here"
    );
    assert_eq!(page.matches("<script>").count(), 1, "one script runs");
    assert!(page.contains(&format!("script-src 'sha384-{SCRIPT_SHA384}'\"")));
    let policy = page
        .split("script-src")
        .nth(1)
        .and_then(|s| s.split('"').next());
    assert!(!policy.unwrap_or_default().contains("unsafe"), "{policy:?}");
    assert!(!page.contains("{{"), "every place is filled");
}

#[test]
fn the_agents_reach_the_page_as_data_that_never_ends_its_element() {
    let (marks, installed) = (
        r#"{"x":"</script><script>alert(1)</script>"}"#,
        r#"[{"n":"<b>"}]"#,
    );
    let page = page(marks, installed);
    // JSON's own escape of `<`, which the script's parse reads back as it was.
    let escaped = |json: &str| json.replace('<', concat!("\\", "u003c"));
    assert!(page.contains(&format!("id=\"marks\">{}</script>", escaped(marks))));
    assert!(page.contains(&format!("id=\"agents\">{}</script>", escaped(installed))));
    assert_eq!(page.matches("<script>").count(), 1);
}
