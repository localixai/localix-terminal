//! The window's dialog: one shown at a time, a question asked twice shown once, a question with
//! a second act answered by any of its three buttons, and a release's notes read as the lines a
//! person sees.

use std::sync::{Arc, Mutex};

use super::{Answer, Dialog, Note, Stage, notes, said, script};
use crate::browser::Confirm;
use crate::kind::{Field, Given, Question};

fn about(title: &str) -> Dialog {
    Dialog::question(&Confirm {
        title: title.into(),
        detail: "Two terminals still run a program.".into(),
        yes: "Quit".into(),
        other: None,
    })
}

#[test]
fn a_question_is_a_dialog_with_its_act_and_cancel_that_takes_the_keys() {
    let dialog = about("Quit Localix Terminal?");
    assert_eq!(
        (dialog.yes.as_str(), dialog.no.as_str(), dialog.asked),
        ("Quit", "Cancel", true)
    );
    assert_eq!(
        dialog.other, None,
        "a question with no second act has two buttons"
    );
    let drawn = script(Some(&dialog));
    assert!(drawn.starts_with("DLG({"), "{drawn}");
    assert!(
        drawn.contains("\"title\":\"Quit Localix Terminal?\""),
        "{drawn}"
    );
    assert!(!drawn.contains("ask:"), "its id is not the page's: {drawn}");
    assert_eq!(script(None), "DLG(null)");
}

#[test]
fn dialogs_are_shown_one_at_a_time_and_one_asked_again_is_not_shown_twice() {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let hear = |who: &'static str| {
        let heard = Arc::clone(&heard);
        Box::new(move |answer: Answer, _: &Given| crate::lock(&heard).push((who, answer)))
            as super::Done
    };
    let mut stage = Stage::default();
    stage.ask(about("Quit Localix Terminal?"), hear("first"));
    stage.ask(about("Restart Localix Terminal to update?"), hear("other"));
    stage.ask(about("Quit Localix Terminal?"), hear("again"));
    assert_eq!(stage.up.len(), 2, "the same question joins the one up");
    let shown = stage.up.front().map(|(dialog, _)| dialog.title.clone());
    assert_eq!(shown.as_deref(), Some("Quit Localix Terminal?"));
    for done in stage.answered() {
        done(Answer::Yes, &Given::new());
    }
    assert_eq!(
        *crate::lock(&heard),
        [("first", Answer::Yes), ("again", Answer::Yes)]
    );
    for done in stage.answered() {
        done(Answer::No, &Given::new());
    }
    assert_eq!(crate::lock(&heard).last(), Some(&("other", Answer::No)));
    assert!(stage.answered().is_empty(), "none up: nobody waits");
}

#[test]
fn a_releases_notes_are_read_as_headings_points_and_sentences_in_plain_words() {
    let read = notes(
        "The window asks in its own dialog.\n\n### Added\n\n- **Dialogs**: one look, see\n  \
         [the rules](docs/ui.md).\n- `llxterm open` runs on its own.\n\n## Fixed\n* A tab.",
    );
    assert_eq!(
        read,
        [
            Note::Text("The window asks in its own dialog.".into()),
            Note::Head("Added".into()),
            Note::Item("Dialogs: one look, see the rules.".into()),
            Note::Item("llxterm open runs on its own.".into()),
            Note::Head("Fixed".into()),
            Note::Item("A tab.".into()),
        ]
    );
    assert_eq!(
        notes("- a [bracket alone"),
        [Note::Item("a [bracket alone".into())]
    );
    assert!(notes(&"- point\n".repeat(200)).len() <= 40, "held short");
}

#[test]
fn the_pages_answer_is_yes_its_other_act_or_no_with_what_was_typed_and_nothing_else() {
    assert_eq!(said("dialog yes"), Some((Answer::Yes, Given::new())));
    assert_eq!(said("dialog other"), Some((Answer::Other, Given::new())));
    assert_eq!(said("dialog no"), Some((Answer::No, Given::new())));
    let typed = said(r#"dialog yes {"API_KEY":"s3cret"}"#);
    let given = Given::from([("API_KEY".to_string(), "s3cret".to_string())]);
    assert_eq!(typed, Some((Answer::Yes, given)));
    assert_eq!(
        said(r#"dialog other {"API_KEY":"s3cret"}"#),
        None,
        "only yes types"
    );
    assert_eq!(said("dialog yes {not json"), None);
    assert_eq!(said(r#"dialog yes {"n":1}"#), None, "only words are typed");
    assert_eq!(said("dialog"), None);
    assert_eq!(said("offer take"), None);
}

#[test]
fn a_tabs_question_is_the_same_dialog_with_its_points_and_fields() {
    let field = Field {
        name: "API_KEY".into(),
        label: "API_KEY".into(),
        secret: true,
        required: true,
        ..Field::default()
    };
    let question = Question {
        title: "Install Weather in Claude Code?".into(),
        detail: "It adds this to the agent.".into(),
        heading: "What It Does".into(),
        points: vec!["Adds the server to ~/.claude.json".into()],
        fields: vec![field],
        yes: "Install".into(),
        ..Question::default()
    };
    let dialog = Dialog::asking(&question);
    assert_eq!((dialog.tone, dialog.no.as_str()), ("neutral", "Cancel"));
    assert!(!dialog.asked, "one an agent raised leaves the keys alone");
    assert_eq!(
        dialog.notes,
        [Note::Item("Adds the server to ~/.claude.json".into())]
    );
    let drawn = script(Some(&dialog));
    assert!(drawn.contains("\"heading\":\"What It Does\""), "{drawn}");
    assert!(drawn.contains("\"secret\":true"), "{drawn}");
    let gone = Question {
        danger: true,
        asked: true,
        ..question
    };
    let dialog = Dialog::asking(&gone);
    assert_eq!((dialog.tone, dialog.asked), ("danger", true));
}

#[test]
fn a_question_with_a_second_act_shows_it_as_a_third_button_and_only_its_yes_goes_on() {
    let dialog = Dialog::question(&Confirm {
        title: "Open Link?".into(),
        detail: "https://example.com/a".into(),
        yes: "Open in Tab".into(),
        other: Some("Open in Browser".into()),
    });
    assert_eq!(
        (
            dialog.yes.as_str(),
            dialog.other.as_deref(),
            dialog.no.as_str()
        ),
        ("Open in Tab", Some("Open in Browser"), "Cancel")
    );
    let drawn = script(Some(&dialog));
    assert!(drawn.contains("\"other\":\"Open in Browser\""), "{drawn}");
    assert!(Answer::Yes.yes() && !Answer::Other.yes() && !Answer::No.yes());
}
