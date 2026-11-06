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
