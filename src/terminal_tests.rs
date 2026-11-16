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

use crate::page::digest;

#[test]
fn the_page_leaves_the_terminals_name_to_the_session() {
    // XTVERSION is answered by the session (session/queries.rs): an xterm.js that answered it
    // too would type a second reply into the program's input.
    assert!(!XTERM.contains(r#"registerCsiHandler({prefix:">",final:"q"}"#));
    assert!(XTERM.contains(r#"registerCsiHandler({prefix:">",final:"c"}"#));
}

#[test]
fn the_terminal_page_runs_the_vendored_scripts_by_their_hashes_and_nothing_else() {
    let page = page("session 2 · <i>");
    assert!(page.contains(&format!("<script>{XTERM}</script>")));
    assert!(page.contains(&format!("<script>{SEARCH}</script>")));
    assert!(page.contains(&format!("<script>{GLUE}</script>")));
    let policy = page
        .split("content=\"default-src 'none'; style-src 'unsafe-inline'; script-src ")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap();
    assert_eq!(
        policy,
        format!("'sha384-{XTERM_SHA384}' 'sha384-{SEARCH_SHA384}' 'sha384-{GLUE_SHA384}'")
    );
    assert!(page.contains("default-src 'none'"));
    assert!(page.contains("<title>session 2 · &lt;i&gt;</title>"));
    assert!(!page.contains("{{scripts}}") && !page.contains("{{glue}}"));
}

#[test]
fn every_script_is_the_one_its_hash_names() {
    assert_eq!(XTERM_SHA384, digest(XTERM));
    assert_eq!(SEARCH_SHA384, digest(SEARCH));
    assert_eq!(GLUE_SHA384, digest(GLUE));
}

#[test]
fn nothing_vendored_can_end_its_own_element() {
    for text in [XTERM, XTERM_CSS, SEARCH, GLUE] {
        let text = text.to_ascii_lowercase();
        assert!(!text.contains("</script") && !text.contains("</style"));
    }
}

#[test]
fn the_page_takes_the_keys_and_the_size() {
    // The glue posts what panel.rs reads: keys, pastes, size when ready and when it changes.
    for post in [
        "\"k\" + data",
        "sized(\"r\")",
        "sized(\"s\")",
        "term.resize(",
    ] {
        assert!(GLUE.contains(post), "{post}");
    }
}

#[test]
fn option_drag_selects_even_where_a_program_took_the_mouse() {
    assert!(GLUE.contains("macOptionClickForcesSelection: true"));
}

#[test]
fn the_page_finds_in_the_output_with_the_search_addon() {
    // The addon lights every match through xterm's decorations, which xterm keeps behind its
    // proposed API; the page's own find bar drives it, and the window says only which step.
    assert!(SEARCH.contains("SearchAddon"));
    for said in [
        "allowProposedApi: true",
        "new SearchAddon.SearchAddon(",
        "function findText(",
    ] {
        assert!(GLUE.contains(said), "{said}");
    }
    let page = page("session 3");
    for part in ["id=\"find\"", "role=\"status\"", "aria-label=\"Find\""] {
        assert!(page.contains(part), "{part}");
    }
}
