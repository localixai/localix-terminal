//! Tests of `script`.

use super::*;

#[test]
fn the_last_statement_after_a_block_is_returned_too() {
    let forms = script_forms("let n = 0; while (n < 3) { n++ } n * 2");
    assert!(
        forms.iter().any(|form| form
            .tail
            .contains("while (n < 3) { n++ }\nreturn (n * 2\n)")),
        "the last statement, returned"
    );
    assert!(
        forms.last().unwrap().tail.contains("{ n++ } n * 2\n}"),
        "the body as it is, last"
    );
}

#[test]
fn an_object_literal_after_a_block_still_finds_the_block() {
    let forms = script_forms("let n = 0; while (n < 3) { n++ } JSON.stringify({ n })");
    assert!(
        forms.iter().any(|form| form
            .tail
            .contains("{ n++ }\nreturn (JSON.stringify({ n })\n)")),
        "the object literal, returned"
    );
}

#[test]
fn only_the_call_is_filled_in_and_the_callers_text_is_left_as_it_is() {
    let forms = script_forms("document.title = '__ID__'");
    let script = forms[0].at(&head("k3y", 42, "w0rd"));
    assert!(
        script.starts_with(
            "window.__dloop_call ? window.__dloop_call(\"k3y\", \"42\", \"w0rd\", \"settled\""
        ),
        "{script}"
    );
    assert!(script.contains("document.title = '__ID__'"), "{script}");
}

#[test]
fn a_call_carries_its_id_as_text_whole() {
    // Past 2^53 a page's number would round the id, and the reply would name another call.
    let id = u64::MAX - 1;
    assert!(head("k", id, "w").contains(&format!("\"{id}\"")));
}
