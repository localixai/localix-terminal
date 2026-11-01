//! Tests of `openrouter`.

use super::*;

#[test]
fn a_choice_and_a_noul_are_read_from_the_wire() {
    let choice = json!({"type": "choice", "choice": "a", "probabilities": {"a": 0.9, "b": 0.1}, "confidence": 0.8});
    assert_eq!(
        read_answer(&choice),
        Some(Answer::Choice {
            choice: "a".into(),
            probabilities: BTreeMap::from([("a".into(), 0.9), ("b".into(), 0.1)]),
            confidence: 0.8,
        })
    );
    assert_eq!(
        read_answer(&json!({"type": "noul", "noul": 0.25})),
        Some(Answer::Noul(0.25))
    );
    assert_eq!(read_answer(&json!({"type": "other"})), None);
}

#[test]
fn a_question_is_sent_with_its_type() {
    let q = Question::Noul {
        instructions: json!("done?"),
    };
    assert_eq!(
        wire_question(&q),
        json!({"type": "noul", "instructions": "done?"})
    );
}

#[test]
fn https_goes_through_the_tls_that_is_built_in() {
    // Rustls is not compiled in: an agent left on ureq's default panics on the first https call.
    let model = OpenRouter::new("key", None);
    assert_eq!(
        model.agent.config().tls_config().provider(),
        ureq::tls::TlsProvider::NativeTls
    );
}
