//! The window's dialog: one shown at a time, a question asked twice shown once, and a release's
//! notes read as the lines a person sees.

use std::sync::{Arc, Mutex};

use super::{Dialog, Note, Stage, notes, said, script};
use crate::browser::Confirm;

fn about(title: &str) -> Dialog {
    Dialog::question(&Confirm {
        title: title.into(),
        detail: "Two terminals still run a program.".into(),
        yes: "Quit".into(),
    })
}

#[test]
fn a_question_is_a_dialog_with_its_act_and_cancel_that_takes_the_keys() {
    let dialog = about("Quit LLX Desk?");
    assert_eq!(
        (dialog.yes.as_str(), dialog.no.as_str(), dialog.asked),
        ("Quit", "Cancel", true)
    );
    let drawn = script(Some(&dialog));
    assert!(drawn.starts_with("DLG({"), "{drawn}");
    assert!(drawn.contains("\"title\":\"Quit LLX Desk?\""), "{drawn}");
    assert!(!drawn.contains("ask:"), "its id is not the page's: {drawn}");
    assert_eq!(script(None), "DLG(null)");
}

#[test]
fn dialogs_are_shown_one_at_a_time_and_one_asked_again_is_not_shown_twice() {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let hear = |who: &'static str| {
        let heard = Arc::clone(&heard);
        Box::new(move |yes: bool| crate::lock(&heard).push((who, yes))) as Box<dyn FnOnce(bool)>
    };
    let mut stage = Stage::default();
    stage.ask(about("Quit LLX Desk?"), hear("first"));
    stage.ask(about("Restart LLX Desk to update?"), hear("other"));
    stage.ask(about("Quit LLX Desk?"), hear("again"));
    assert_eq!(stage.up.len(), 2, "the same question joins the one up");
    let shown = stage.up.front().map(|(dialog, _)| dialog.title.clone());
    assert_eq!(shown.as_deref(), Some("Quit LLX Desk?"));
    for done in stage.answered() {
        done(true);
    }
    assert_eq!(*crate::lock(&heard), [("first", true), ("again", true)]);
    for done in stage.answered() {
        done(false);
    }
    assert_eq!(crate::lock(&heard).last(), Some(&("other", false)));
    assert!(stage.answered().is_empty(), "none up: nobody waits");
}

#[test]
fn a_releases_notes_are_read_as_headings_points_and_sentences_in_plain_words() {
    let read = notes(
        "The window asks in its own dialog.\n\n### Added\n\n- **Dialogs**: one look, see\n  \
         [the rules](docs/ui.md).\n- `llx-desk setup` runs on its own.\n\n## Fixed\n* A tab.",
    );
    assert_eq!(
        read,
        [
            Note::Text("The window asks in its own dialog.".into()),
            Note::Head("Added".into()),
            Note::Item("Dialogs: one look, see the rules.".into()),
            Note::Item("llx-desk setup runs on its own.".into()),
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
fn the_pages_answer_is_yes_or_no_and_nothing_else() {
    assert_eq!(said("dialog yes"), Some(true));
    assert_eq!(said("dialog no"), Some(false));
    assert_eq!(said("dialog"), None);
    assert_eq!(said("offer take"), None);
}
