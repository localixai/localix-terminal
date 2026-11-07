//! Tests of `call`.

use super::*;
use crate::testing::Scratch;
use crate::wire::Blob;

#[test]
fn a_picture_that_came_as_bytes_is_saved_and_its_path_printed() {
    let scratch = Scratch::new();
    let answer = Answer {
        value: json!({"text": "w1: scrolled", "shot": {"width": 800, "height": 600}}),
        blob: Some(Blob {
            mime: "image/jpeg".into(),
            bytes: vec![0xff, 0xd8, 1, 2, 3],
        }),
    };
    let said = words(&answer, scratch.path());
    assert!(
        said.starts_with("w1: scrolled\n\nScreenshot saved (800x600): "),
        "{said}"
    );
    let path = said
        .lines()
        .find_map(|line| line.split("(800x600): ").nth(1))
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), vec![0xff, 0xd8, 1, 2, 3]);
    assert!(said.ends_with("Read that file to see it."));
}

#[test]
fn an_answer_without_a_picture_is_its_text_alone() {
    let answer = Answer {
        value: json!({"text": "Closed 2 tabs."}),
        blob: None,
    };
    assert_eq!(
        words(&answer, std::path::Path::new("/nonexistent")),
        "Closed 2 tabs."
    );
}

#[test]
fn peek_marks_a_look_that_leaves_the_persons_notes() {
    let args: Vec<String> = ["--peek", r#"{"action":"list"}"#]
        .map(String::from)
        .to_vec();
    let parsed = parse(&args).unwrap();
    assert!(parsed.peek);
    assert_eq!(parsed.json.as_deref(), Some(r#"{"action":"list"}"#));
    assert!(!parse(&args[1..]).unwrap().peek);
}

fn parsed(words: &[&str]) -> Result<Window, String> {
    parse(
        &words
            .iter()
            .map(|word| (*word).to_string())
            .collect::<Vec<_>>(),
    )
}

#[test]
fn a_flag_that_applies_to_nothing_is_refused_and_the_plugins_calls_still_parse() {
    // As the Claude Code plugin calls it.
    assert!(parsed(&["--project", "/x", "--said", "--follow", "--json"]).is_ok());
    assert!(parsed(&["--project", "/x", "--end"]).is_ok());
    assert!(parsed(&["--as", "me", "--board"]).is_ok());
    // `--said --json` follows, as it always did.
    assert!(parsed(&["--said", "--json"]).is_ok());
    assert!(parsed(&["--json"]).is_err());
    assert!(parsed(&["--follow"]).is_err());
    assert!(parsed(&["--end", "--follow"]).is_err());
    // A JSON argument is never dropped by a mode that takes none.
    assert!(parsed(&["--end", r#"{"action":"close"}"#]).is_err());
    assert!(parsed(&["--said", "{}"]).is_err());
    assert!(parsed(&["--board", "{}"]).is_err());
    // A name or a folder is not the next option.
    assert!(parsed(&["--as", "--end"]).is_err());
    assert!(parsed(&["--project", "--said"]).is_err());
}

#[test]
fn stdin_past_its_limit_is_refused_and_a_bad_object_says_why() {
    assert_eq!(read_up_to(&b"{}"[..], 2).unwrap(), "{}");
    let why = read_up_to(&b"{} "[..], 2).unwrap_err();
    assert!(why.contains("too large"), "{why}");
    let why = object("{\"action\":").unwrap_err();
    assert!(why.contains("EOF"), "{why}");
    assert!(object("[1]").is_err());
    assert!(object("{}").is_ok());
}
