//! Tests of `catalog::source::frontmatter`, against the ways a `SKILL.md` writes its keys.

use super::*;

#[test]
fn plain_and_quoted_values_are_read_and_the_body_is_not() {
    let keys = read(
        "---\nname: pdf\ndescription: \"Fill forms: \\\"PDF\\\" files\"\nlicense: 'It''s free'\n\
         metadata:\n  version: 2\n---\n# PDF\nname: not a key\n",
    );
    assert_eq!(keys["name"], "pdf");
    assert_eq!(keys["description"], "Fill forms: \"PDF\" files");
    assert_eq!(keys["license"], "It's free");
    // A nested map is passed over as a key of its own; its inner keys are not the top's.
    assert!(!keys.contains_key("version"));
    assert_eq!(keys.len(), 4);
}

#[test]
fn a_folded_or_kept_block_and_a_plain_value_on_lines_after_are_read_whole() {
    let keys = read(
        "---\r\nname: docx\r\ndescription: >-\r\n  Create and edit\r\n  Word documents.\r\n\
         notes: |\r\n  one\r\n  two\r\nsummary: Long words\r\n  that go on\r\n---\r\n",
    );
    assert_eq!(keys["description"], "Create and edit Word documents.");
    assert_eq!(keys["notes"], "one\ntwo");
    assert_eq!(keys["summary"], "Long words that go on");
}

#[test]
fn a_file_with_no_frontmatter_has_no_keys() {
    assert!(read("# Title\nname: x\n").is_empty());
    assert!(read("").is_empty());
    // A frontmatter that never ends is read to the end of the file.
    assert_eq!(read("---\nname: x")["name"], "x");
}
