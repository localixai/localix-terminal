//! A recording as an emulator reads it again: a header, then the output and the resizes in the
//! order they came, the text whole however the reads cut it, in a file only its user reads.

use std::os::unix::fs::PermissionsExt;

use super::Record;
use crate::testing::Scratch;

fn lines(record: &Record) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(record.path()).unwrap();
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The text of every output event, joined, and the resizes.
fn played(events: &[serde_json::Value]) -> (String, Vec<String>) {
    let mut output = String::new();
    let mut sizes = vec![];
    for event in events {
        let text = event[2].as_str().unwrap().to_string();
        match event[1].as_str() {
            Some("o") => output.push_str(&text),
            Some("r") => sizes.push(text),
            other => panic!("an event of kind {other:?}"),
        }
    }
    (output, sizes)
}

#[test]
fn a_recording_is_its_header_then_the_output_and_resizes_as_they_came() {
    let scratch = Scratch::new();
    let mut record = Record::open(scratch.path(), 40, 120).unwrap();
    record.output(b"hi \x1b[31mred\x1b[0m\r\n").unwrap();
    record.resize(24, 80).unwrap();
    record.output(b"after").unwrap();
    let lines = lines(&record);
    assert_eq!(lines[0]["version"], 2);
    assert_eq!(lines[0]["width"], 120);
    assert_eq!(lines[0]["height"], 40);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[1][1], "o");
    assert_eq!(lines[1][2], "hi \u{1b}[31mred\u{1b}[0m\r\n");
    assert_eq!(lines[2][1], "r");
    assert_eq!(lines[2][2], "80x24");
    assert_eq!(lines[3][2], "after");
    let times: Vec<f64> = lines[1..].iter().map(|l| l[0].as_f64().unwrap()).collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "{times:?}");
}

#[test]
fn a_character_cut_between_two_reads_is_written_whole() {
    let scratch = Scratch::new();
    let mut record = Record::open(scratch.path(), 40, 120).unwrap();
    let text = "é👍".as_bytes();
    record.output(&text[..1]).unwrap();
    record.output(&text[1..4]).unwrap();
    record.output(&text[4..]).unwrap();
    let (output, _) = played(&lines(&record)[1..]);
    assert_eq!(output, "é👍");
}

#[test]
fn bytes_that_are_no_text_are_written_as_the_replacement_character() {
    let scratch = Scratch::new();
    let mut record = Record::open(scratch.path(), 40, 120).unwrap();
    record.output(b"a\xffb\xc3").unwrap();
    record.output(b"(").unwrap();
    let (output, _) = played(&lines(&record)[1..]);
    assert_eq!(output, "a\u{fffd}b\u{fffd}(");
}

#[test]
fn the_file_is_its_users_alone_and_each_terminal_has_its_own() {
    let scratch = Scratch::new();
    let first = Record::open(scratch.path(), 40, 120).unwrap();
    let second = Record::open(scratch.path(), 40, 120).unwrap();
    assert_ne!(first.path(), second.path());
    let mode = std::fs::metadata(first.path())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert!(first.path().extension().is_some_and(|ext| ext == "cast"));
}

#[test]
fn a_folder_that_is_not_there_yet_is_made() {
    let scratch = Scratch::new();
    let record = Record::open(&scratch.path().join("deeper/still"), 40, 120).unwrap();
    assert!(record.path().exists());
}
