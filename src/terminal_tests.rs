//! Tests of `terminal`.

use super::*;

#[test]
fn the_page_is_in_the_windows_light_or_dark() {
    let page = page("session 1 · <b>&");
    assert!(
        page.contains("<title>session 1 · &lt;b&gt;&amp;</title>"),
        "{page}"
    );
    assert!(page.contains("Content-Security-Policy"));
    assert!(page.contains("prefers-color-scheme: dark"));
    assert!(!page.contains("{{theme}}"));
}

fn digest(text: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha384};
    base64::engine::general_purpose::STANDARD.encode(Sha384::digest(text))
}

#[test]
fn the_terminal_page_runs_the_vendored_scripts_by_their_hashes_and_nothing_else() {
    let page = page("session 2 · <i>");
    assert!(page.contains(&format!("<script>{XTERM}</script>")));
    assert!(page.contains(&format!("<script>{FIT}</script>")));
    assert!(page.contains(&format!("<script>{GLUE}</script>")));
    let policy = page
        .split("content=\"default-src 'none'; style-src 'unsafe-inline'; script-src ")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap();
    assert_eq!(
        policy,
        format!("'sha384-{XTERM_SHA384}' 'sha384-{FIT_SHA384}' 'sha384-{GLUE_SHA384}'")
    );
    assert!(page.contains("default-src 'none'"));
    assert!(page.contains("<title>session 2 · &lt;i&gt;</title>"));
    assert!(!page.contains("{{scripts}}") && !page.contains("{{glue}}"));
}

#[test]
fn every_script_is_the_one_its_hash_names() {
    assert_eq!(XTERM_SHA384, digest(XTERM));
    assert_eq!(FIT_SHA384, digest(FIT));
    assert_eq!(GLUE_SHA384, digest(GLUE));
}

#[test]
fn nothing_vendored_can_end_its_own_element() {
    for text in [XTERM, FIT, XTERM_CSS, GLUE] {
        let text = text.to_ascii_lowercase();
        assert!(!text.contains("</script") && !text.contains("</style"));
    }
}

#[test]
fn the_page_takes_the_keys_and_the_size() {
    // The glue posts what panel.rs reads: keys, pastes, size when ready and when it changes.
    for post in ["\"k\" + data", "sized(\"r\")", "sized(\"s\")", "fit.fit()"] {
        assert!(GLUE.contains(post), "{post}");
    }
}
