//! Tests of `page`.

use super::*;

#[test]
fn the_page_runs_its_one_script_by_its_hash_and_its_title_is_text() {
    let page = page("board · <b>shop</b>");
    let digest = crate::page::digest(SCRIPT);
    assert_eq!(
        SCRIPT_SHA384, digest,
        "page.js changed: put its new SHA-384 here"
    );
    assert!(page.contains(&format!("script-src 'sha384-{SCRIPT_SHA384}'")));
    assert!(page.contains("default-src 'none'"));
    assert!(page.contains(&format!("<script>{SCRIPT}</script>")));
    assert!(page.contains("<title>board · &lt;b&gt;shop&lt;/b&gt;</title>"));
    assert!(!page.contains("{{"), "every place is filled");
    assert!(!SCRIPT.to_ascii_lowercase().contains("</script"));
    assert!(
        !SCRIPT.contains("innerHTML"),
        "what the board holds is set as text"
    );
}

#[test]
fn a_frame_carries_the_board_whole_as_data() {
    let mut board = Board::default();
    board
        .add("say </script> \"hi\"", "", None, "agent")
        .unwrap();
    let agents = serde_json::json!([{"id": "codex", "name": "Codex"}]);
    let frame = frame(&board, "b1", "shop", 42, &agents);
    let state: Value =
        serde_json::from_str(frame.strip_prefix("B(").unwrap().strip_suffix(')').unwrap()).unwrap();
    assert_eq!(state["cards"][0]["title"], "say </script> \"hi\"");
    assert_eq!(
        (state["key"].as_str(), state["now"].as_u64()),
        (Some("b1"), Some(42))
    );
    assert_eq!(state["agents"][0]["id"], "codex");
    assert_eq!(refused("no \"card\""), r#"E("no \"card\"")"#);
}
