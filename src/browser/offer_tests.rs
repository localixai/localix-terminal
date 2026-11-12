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
        id: "integration:claude-code".into(),
        agent: Some("claude"),
        says: "Claude Code runs here.".into(),
        take: Some("Add Plugin".into()),
        later: Some("Not Now".into()),
        busy: false,
        failed: false,
    }
}

#[test]
fn taking_an_offer_holds_the_bar_busy_until_it_is_done() {
    let offers = Offers::default();
    let bar = asked();
    assert_eq!(
        offers.clicked(Some(&bar), true).as_deref(),
        Some("integration:claude-code")
    );
    let held = crate::lock(&offers.held).clone().unwrap();
    assert!(
        held.busy && held.take.is_none() && held.later.is_none(),
        "{held:?}"
    );
    // A busy bar has nothing to click; none at all is nothing to do.
    assert_eq!(offers.clicked(Some(&held), true), None);
    assert_eq!(offers.clicked(None, true), None);
    // Failed, it says why and offers to try again.
    offers.done(
        "integration:claude-code",
        Err("claude was not found".into()),
    );
    let failed = crate::lock(&offers.held).clone().unwrap();
    assert!(failed.failed && failed.says.contains("claude was not found"));
    assert_eq!(failed.take.as_deref(), Some("Try Again"));
}

#[test]
fn the_bar_is_drawn_as_text() {
    let drawn = script(Some(&asked()));
    assert!(
        drawn.starts_with("O({") && drawn.contains("\"take\":\"Add Plugin\""),
        "{drawn}"
    );
    assert!(
        !drawn.contains("integration:"),
        "the id stays the window's: {drawn}"
    );
    assert_eq!(script(None), "O(null)");
}

#[test]
fn an_agent_starting_reads_its_integration_again_and_settings_tell_what_they_read() {
    let offers = Offers::default();
    // Read on once: the plugin was installed.
    offers.known("claude-code", true);
    // Taken out since (in Settings, or with `claude plugin uninstall`): Claude starting in a
    // terminal reads it again, instead of what was known.
    assert!(offers.runs("t1", Some("claude")));
    assert!(!crate::lock(&offers.on).contains_key("claude-code"));
    // What Settings read is what the bar knows.
    offers.read_all(&serde_json::json!([
        {"id": "claude-code", "state": "off", "declined": false},
        {"id": "codex", "state": "on", "declined": false},
        {"id": "gemini", "state": "unavailable", "declined": false},
        {"id": "hermes", "state": "off", "declined": true},
    ]));
    let on = crate::lock(&offers.on).clone();
    // Offered: off, and not taken out by the person.
    assert_eq!(on.get("claude-code"), Some(&Some(false)));
    assert_eq!(on.get("codex"), Some(&Some(true)));
    assert_eq!(on.get("gemini"), Some(&Some(true)));
    assert_eq!(on.get("hermes"), Some(&Some(true)));
    assert!(!on.contains_key("copilot"));
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
fn an_update_taken_is_the_desk_s_to_carry_out_and_one_put_off_closes() {
    let offers = Offers::default();
    let bar = Bar {
        id: "update:0.2.0".to_string(),
        agent: None,
        says: "Localix Terminal 0.2.0 is ready to install.".to_string(),
        take: Some("Restart to Update".to_string()),
        later: Some("Not Now".to_string()),
        busy: false,
        failed: false,
    };
    // Taken: its id goes on, and nothing is held (the desk may ask first, and the app ends).
    assert_eq!(
        offers.clicked(Some(&bar), true).as_deref(),
        Some("update:0.2.0")
    );
    assert_eq!(*crate::lock(&offers.held), None);
    assert_eq!(offers.clicked(Some(&bar), false), None);
    assert_eq!(*crate::lock(&offers.held), None);
}
