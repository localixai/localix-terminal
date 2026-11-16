//! Tests of `offer`.

use super::*;

#[test]
fn a_terminal_runs_an_agent_by_its_program_and_says_when_it_changes() {
    let offers = Offers::default();
    assert!(offers.runs("t1", Some("claude")));
    assert_eq!(offers.agent("t1"), Some("claude"));
    // The same again is no change; a shell in front is no agent.
    assert!(!offers.runs("t1", Some("claude")));
    assert!(offers.runs("t1", Some("zsh")));
    assert_eq!(offers.agent("t1"), None);
    assert!(!offers.runs("t1", None));
    assert!(offers.runs("t2", Some("codex")) && offers.agent("t2") == Some("codex"));
}

fn asked() -> Bar {
    Bar {
        id: "update:0.2.0".to_string(),
        says: "Localix Terminal 0.2.0 is ready to install.".to_string(),
        take: Some("Restart to Update".to_string()),
        later: Some("Not Now".to_string()),
    }
}

#[test]
fn the_bar_is_drawn_as_text() {
    let drawn = script(Some(&asked()));
    assert!(
        drawn.starts_with("O({") && drawn.contains("\"take\":\"Restart to Update\""),
        "{drawn}"
    );
    assert!(
        !drawn.contains("update:"),
        "the id stays the window's: {drawn}"
    );
    assert_eq!(script(None), "O(null)");
}

/// Answers that count how often the store is asked.
struct Counted(std::sync::atomic::AtomicUsize);

impl super::super::Answers for Counted {
    fn due(&self, _: &str) -> bool {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        true
    }

    fn answered(&self, _: &str, _: bool) {}
}

#[test]
fn the_store_is_asked_once_for_the_refreshes_of_a_moment_and_an_answer_ends_the_offer_at_once() {
    let offers = Offers::default();
    assert!(
        !offers.due("x"),
        "nothing is offered before the answers are known"
    );
    let counted = Arc::new(Counted(std::sync::atomic::AtomicUsize::new(0)));
    let answers: Arc<dyn super::super::Answers> = Arc::clone(&counted) as _;
    offers.keep(answers);
    assert!(offers.due("x") && offers.due("x") && offers.due("x"));
    assert_eq!(counted.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    offers.answered("x", false);
    assert!(
        !offers.due("x"),
        "put off, it is not due, before the store is written"
    );
}

#[test]
fn an_update_taken_is_the_desk_s_to_carry_out_and_one_put_off_waits() {
    let offers = Offers::default();
    let bar = asked();
    // Taken: its id goes on (the desk may ask first, and the app ends).
    assert_eq!(
        offers.clicked(Some(&bar), true).as_deref(),
        Some("update:0.2.0")
    );
    assert_eq!(offers.clicked(None, true), None);
    // Put off: nothing to do, and it is no longer due.
    let counted = Arc::new(Counted(std::sync::atomic::AtomicUsize::new(0)));
    offers.keep(Arc::clone(&counted) as _);
    assert_eq!(offers.clicked(Some(&bar), false), None);
    assert!(!offers.due("update:0.2.0"));
}
