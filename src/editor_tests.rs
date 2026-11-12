//! Tests of `editor`: a page runs the core and its file's language by their hashes and nothing
//! else, a file's language is told from its name, and the page's messages are read.

use super::vendored::SCRIPTS;
use super::*;

use crate::page::digest;

fn policy(page: &str) -> &str {
    page.split("content=\"default-src 'none'; style-src 'unsafe-inline'; script-src ")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap()
}

#[test]
fn a_page_runs_the_core_and_its_files_language_by_their_hashes_and_nothing_else() {
    let hash = |name: &str| format!("'sha384-{}'", vendored(name).unwrap().sha384);
    let glue = format!("'sha384-{GLUE_SHA384}'");
    let rust = page("main.rs · <b>&", language("main.rs"));
    assert_eq!(
        policy(&rust),
        format!("{} {} {glue}", hash("core"), hash("rust"))
    );
    assert!(rust.contains("<title>main.rs · &lt;b&gt;&amp;</title>"));
    assert_eq!(rust.matches("<script>").count(), 3);
    let html = page("index.html", language("index.html"));
    assert_eq!(
        policy(&html),
        format!(
            "{} {} {} {} {glue}",
            hash("core"),
            hash("javascript"),
            hash("css"),
            hash("html")
        )
    );
    let plain = page("notes.txt", language("notes.txt"));
    assert_eq!(policy(&plain), format!("{} {glue}", hash("core")));
    for page in [&rust, &html, &plain] {
        assert!(!page.contains("{{vendored}}") && !page.contains("{{scripts}}"));
        assert!(page.contains(&format!("<script>{GLUE}</script>")));
    }
}

#[test]
fn every_script_is_the_one_its_hash_names() {
    assert_eq!(GLUE_SHA384, digest(GLUE));
    for script in SCRIPTS {
        assert_eq!(script.sha384, digest(script.text), "{}", script.name);
    }
}

#[test]
fn nothing_a_page_holds_can_end_its_own_element() {
    for text in (SCRIPTS.iter().map(|script| script.text)).chain([GLUE]) {
        let lower = text.to_ascii_lowercase();
        assert!(!lower.contains("</script") && !lower.contains("<!--"));
    }
}

#[test]
fn a_files_language_is_told_from_its_name() {
    let id = |name: &str| language(name).map(|language| language.id);
    assert_eq!(id("main.rs"), Some("rust"));
    assert_eq!(id("App.TSX"), Some("tsx"));
    assert_eq!(id("Cargo.lock"), Some("toml"));
    assert_eq!(id("Dockerfile"), Some("dockerfile"));
    assert_eq!(id("Dockerfile.dev"), Some("dockerfile"));
    assert_eq!(id(".env.local"), Some("properties"));
    assert_eq!(id("build.gradle.kts"), Some("kotlin"));
    assert_eq!(id("LICENSE"), None);
    assert_eq!(id("photo.png"), None);
    assert!(wraps("README.md") && wraps("notes.TXT") && !wraps("main.rs"));
}

#[test]
fn every_language_a_name_tells_is_held_in_scripts_that_exist() {
    for (_, id) in BY_END.iter().chain(BY_NAME) {
        let held = (HELD_IN.iter())
            .find_map(|(ids, scripts)| ids.contains(id).then_some(*scripts))
            .unwrap_or_else(|| std::slice::from_ref(id));
        for script in held {
            assert!(vendored(script).is_some(), "{id}: no script {script}");
        }
    }
}

#[test]
fn the_pages_messages_are_read() {
    assert_eq!(
        did(r#"{"t":"save","text":"a\nb"}"#),
        Some(Did::Save("a\nb".into()))
    );
    assert_eq!(
        did(r#"{"t":"draft","text":""}"#),
        Some(Did::Draft(String::new()))
    );
    assert_eq!(did(r#"{"t":"clean"}"#), Some(Did::Clean));
    assert_eq!(did(r#"{"t":"save"}"#), None);
    assert_eq!(did(r#"{"t":"other"}"#), None);
    assert_eq!(did("not json"), None);
}

#[test]
fn a_file_is_opened_with_its_language_and_what_was_left_unsaved() {
    let frame = opened(&Opened {
        name: "a.md",
        text: "one\n",
        draft: Some("two\n"),
        moved: true,
        line: Some(3),
    });
    let inside = frame
        .strip_prefix("O(")
        .and_then(|rest| rest.strip_suffix(')'));
    let told: Value = serde_json::from_str(inside.unwrap()).unwrap();
    assert_eq!(told["lang"], "markdown");
    assert_eq!(told["wrap"], true);
    assert_eq!(told["text"], "one\n");
    assert_eq!(told["draft"], "two\n");
    assert_eq!(told["moved"], true);
    assert_eq!(told["line"], 3);
    assert_eq!(disk("x"), r#"D({"text":"x"})"#);
    assert_eq!(saved(Saved::Kept), r#"S({"ok":true})"#);
    assert_eq!(saved(Saved::Conflict), r#"S({"conflict":true,"ok":false})"#);
    assert_eq!(saved(Saved::Failed("no")), r#"S({"ok":false,"why":"no"})"#);
}
