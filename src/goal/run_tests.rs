//! Tests of `run`.

use super::*;

/// The app's pause, as the binary gives it: a test's view has no change event either.
#[expect(
    clippy::disallowed_methods,
    reason = "a poll, as the app's: a view has no change event"
)]
fn pause(time: Duration) {
    std::thread::sleep(time);
}

#[test]
fn a_loop_is_a_state_seen_again_not_a_repeated_action() {
    // Open search, close it, open it again: the third visit to the same state is a loop.
    let (closed, open) = (1, 2);
    assert!(!cycling(&[closed, open], closed));
    assert!(cycling(&[closed, open, closed, open], closed));
    // "Scroll right" five times, each showing new dates: never a loop.
    assert!(!cycling(&[10, 11, 12, 13], 14));
}

mod covered {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use serde_json::Value;

    use super::*;
    use crate::model::DecideError;
    use crate::model::{Answer, Decision, Question};
    use crate::view::{Action, Element, View, ViewResult};

    /// A view whose first click is refused because a banner came over it after the snapshot.
    struct Banner {
        acts: Mutex<usize>,
    }

    fn view(question: usize) -> Observation {
        Observation {
            outline: String::new(),
            epoch: "1".into(),
            location: "http://x".into(),
            title: "Quiz".into(),
            text: format!("Question {question}"),
            scroll: 0,
            elements: vec![Element {
                reference: "e1".into(),
                role: "button".into(),
                name: "Next".into(),
                value: None,
                states: vec![],
                editable: false,
                dialog: false,
                options: vec![],
                form: None,
            }],
            busy: false,
            revision: 0,
            notices: vec![],
        }
    }

    impl View for Banner {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn observe(&self) -> ViewResult<Observation> {
            // The click that went through moved on to the next question.
            let acts = *self.acts.lock().unwrap();
            Ok(view(if acts >= 2 { 2 } else { 1 }))
        }
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn act(&self, _: &str, _: &Action) -> ViewResult<()> {
            let mut acts = self.acts.lock().unwrap();
            *acts += 1;
            if *acts == 1 {
                return Err(ViewError::Refused(
                    "covered: iframe \"Consent\" is on top".into(),
                ));
            }
            Ok(())
        }
    }

    /// Clicks Next until it has clicked once, then says DONE.
    struct Model {
        clicked: Mutex<usize>,
    }

    fn sure(choice: &str) -> Answer {
        Answer::Choice {
            choice: choice.into(),
            probabilities: [(choice.to_string(), 1.0)].into(),
            confidence: 1.0,
        }
    }

    impl Decider for Model {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn decide(
            &self,
            state: &Value,
            _: &BTreeMap<String, Question>,
        ) -> Result<Decision, DecideError> {
            let mut clicked = self.clicked.lock().unwrap();
            let done = state["recent_actions"]
                .as_array()
                .is_some_and(|steps| steps.iter().any(|s| !s.to_string().contains("failed")));
            let answers = if done {
                [("operation".to_string(), sure("DONE"))].into()
            } else {
                *clicked += 1;
                [
                    ("operation".to_string(), sure("CLICK")),
                    ("click_target".to_string(), sure("e1")),
                ]
                .into()
            };
            Ok(Decision {
                answers,
                ..Decision::default()
            })
        }
    }

    #[test]
    fn a_click_under_a_late_banner_is_decided_again_not_the_end() {
        let driver = Banner {
            acts: Mutex::new(0),
        };
        let model = Model {
            clicked: Mutex::new(0),
        };
        let outcome = GoalRun {
            view: &driver,
            decider: &model,
            on_step: &|_| {},
            pause: &pause,
            max_actions: 5,
        }
        .resume("Click Next", &[], vec![]);
        assert_eq!(outcome.stop, Stop::Done { probability: 1.0 });
        assert!(
            outcome.steps[0]
                .failed
                .as_deref()
                .unwrap()
                .starts_with("covered")
        );
        assert_eq!(
            *model.clicked.lock().unwrap(),
            2,
            "clicked again after the refusal"
        );
    }

    /// Says DONE, unsure, every time it is asked.
    struct Unsure {
        asked: Mutex<usize>,
    }

    impl Decider for Unsure {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn decide(
            &self,
            _: &Value,
            _: &BTreeMap<String, Question>,
        ) -> Result<Decision, DecideError> {
            *self.asked.lock().unwrap() += 1;
            let done = Answer::Choice {
                choice: "DONE".into(),
                probabilities: [("DONE".to_string(), 0.55), ("CLICK".to_string(), 0.45)].into(),
                confidence: 0.55,
            };
            Ok(Decision {
                answers: [("operation".to_string(), done)].into(),
                ..Decision::default()
            })
        }
    }

    #[test]
    fn an_unsure_done_on_a_view_that_stays_put_is_not_asked_again() {
        let driver = Banner {
            acts: Mutex::new(0),
        };
        let model = Unsure {
            asked: Mutex::new(0),
        };
        let outcome = GoalRun {
            view: &driver,
            decider: &model,
            on_step: &|_| {},
            pause: &pause,
            max_actions: 5,
        }
        .resume("Answer three questions", &[], vec![]);
        assert_eq!(outcome.stop, Stop::Done { probability: 0.55 });
        assert_eq!(
            *model.asked.lock().unwrap(),
            1,
            "asking again invites a step too many"
        );
    }
}

mod typing {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use serde_json::Value;

    use super::*;
    use crate::model::{Answer, Decision, Question};
    use crate::view::{Action, Element, View, ViewResult};

    /// A terminal's input line: each typing adds to what is there, so the view always changes.
    struct Line {
        typed: Mutex<String>,
    }

    impl View for Line {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn observe(&self) -> ViewResult<Observation> {
            let typed = self.typed.lock().unwrap().clone();
            Ok(Observation {
                outline: String::new(),
                epoch: "1".into(),
                location: "terminal".into(),
                title: "python3".into(),
                text: format!(">>> {typed}"),
                scroll: 0,
                elements: vec![Element {
                    reference: "e1".into(),
                    role: "textbox".into(),
                    name: "terminal input".into(),
                    value: None,
                    states: vec![],
                    editable: true,
                    dialog: false,
                    options: vec![],
                    form: None,
                }],
                busy: false,
                revision: 0,
                notices: vec![],
            })
        }
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn act(&self, _: &str, action: &Action) -> ViewResult<()> {
            if let Action::Type { text, .. } = action {
                self.typed.lock().unwrap().push_str(text);
            }
            Ok(())
        }
    }

    /// Types the one value it was given, whatever the view shows.
    struct Typist;

    impl Decider for Typist {
        fn decide(
            &self,
            _: &Value,
            _: &BTreeMap<String, Question>,
        ) -> Result<Decision, DecideError> {
            let sure = |choice: &str| Answer::Choice {
                choice: choice.into(),
                probabilities: [(choice.to_string(), 1.0)].into(),
                confidence: 1.0,
            };
            Ok(Decision {
                answers: [
                    ("operation".to_string(), sure("TYPE_TEXT")),
                    ("type_entry".to_string(), sure("e1=v1")),
                ]
                .into(),
                ..Decision::default()
            })
        }
    }

    /// A goal the values cannot carry out ("define a function", with only its argument given):
    /// the helper types the value once, then stops instead of typing it again and again, each
    /// time adding it to the line.
    #[test]
    fn the_same_value_typed_into_the_same_field_again_stops_the_run() {
        let line = Line {
            typed: Mutex::new(String::new()),
        };
        let outcome = GoalRun {
            view: &line,
            decider: &Typist,
            on_step: &|_| {},
            pause: &pause,
            max_actions: 40,
        }
        .resume(
            "define a function sq, then call it with the given number",
            &["12".into()],
            vec![],
        );
        assert_eq!(outcome.stop, Stop::Repeated);
        assert_eq!(*line.typed.lock().unwrap(), "12", "typed once");
    }
}

mod keys {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use serde_json::Value;

    use super::*;
    use crate::model::{Answer, DecideError, Decision, Question};
    use crate::view::{Ability, Action, Element, View, ViewResult};

    /// A board moved by the arrow keys alone, as a game takes them.
    #[derive(Default)]
    struct Board {
        pressed: Mutex<Vec<Action>>,
    }

    impl View for Board {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn observe(&self) -> ViewResult<Observation> {
            let moves = self.pressed.lock().unwrap().len();
            Ok(Observation {
                epoch: "1".into(),
                location: "http://game".into(),
                title: "Game".into(),
                text: format!("Moves {moves}"),
                elements: vec![Element {
                    reference: "e1".into(),
                    role: "button".into(),
                    name: "New game".into(),
                    value: None,
                    states: vec![],
                    editable: false,
                    dialog: false,
                    options: vec![],
                    form: None,
                }],
                revision: moves as u64,
                notices: vec![],
                ..Observation::default()
            })
        }

        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn act(&self, _: &str, action: &Action) -> ViewResult<()> {
            self.pressed.lock().unwrap().push(action.clone());
            Ok(())
        }

        fn abilities(&self) -> &'static [Ability] {
            &[Ability::Keys]
        }
    }

    fn sure(choice: &str) -> Answer {
        Answer::Choice {
            choice: choice.into(),
            probabilities: [(choice.to_string(), 1.0)].into(),
            confidence: 1.0,
        }
    }

    /// Presses ArrowLeft, then ArrowDown, until three moves are made; then DONE.
    struct Player;

    impl Decider for Player {
        fn decide(
            &self,
            state: &Value,
            questions: &BTreeMap<String, Question>,
        ) -> Result<Decision, DecideError> {
            let text = state["view"]["text"].as_str().unwrap_or_default();
            let answers = if text == "Moves 3" {
                [("operation".to_string(), sure("DONE"))].into()
            } else {
                if !questions.contains_key("key") {
                    return Err(DecideError::Invalid("no key was offered".into()));
                }
                let key = if text == "Moves 1" {
                    "ArrowDown"
                } else {
                    "ArrowLeft"
                };
                [
                    ("operation".to_string(), sure("PRESS_KEY")),
                    ("key".to_string(), sure(key)),
                ]
                .into()
            };
            Ok(Decision {
                answers,
                ..Decision::default()
            })
        }
    }

    #[test]
    fn a_goal_plays_with_keys_on_a_view_that_takes_them() {
        let board = Board::default();
        let run = GoalRun {
            view: &board,
            decider: &Player,
            on_step: &|_| {},
            pause: &pause,
            max_actions: 10,
        };
        let outcome = run.resume("make three moves", &[], vec![]);
        assert!(
            matches!(outcome.stop, Stop::Done { .. }),
            "{}",
            outcome.stop
        );
        let keys: Vec<Action> = board.pressed.lock().unwrap().clone();
        let press = |key: &str| Action::Press { key: key.into() };
        assert_eq!(
            keys,
            [press("ArrowLeft"), press("ArrowDown"), press("ArrowLeft")]
        );
        assert_eq!(outcome.steps[0].trace_line(), "PRESS_KEY ArrowLeft · 100%");
    }
}

mod futile {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use serde_json::Value;

    use super::*;
    use crate::model::{Answer, DecideError, Decision, Question};
    use crate::view::{Action, Element, View, ViewResult};

    /// A form whose Continue refuses a value: each click shows the same error, and the focus
    /// goes back and forth between the button and the field.
    #[derive(Default)]
    struct Refusing {
        clicks: Mutex<usize>,
    }

    impl View for Refusing {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn observe(&self) -> ViewResult<Observation> {
            let clicks = *self.clicks.lock().unwrap();
            let focused = |on: bool| {
                if on {
                    vec!["focused".to_string()]
                } else {
                    vec![]
                }
            };
            let element = |reference: &str, role: &str, name: &str, on: bool| Element {
                reference: reference.into(),
                role: role.into(),
                name: name.into(),
                value: None,
                states: focused(on),
                editable: role == "textbox",
                dialog: false,
                options: vec![],
                form: None,
            };
            Ok(Observation {
                epoch: "1".into(),
                location: "http://form".into(),
                title: "Form".into(),
                text: "Its address. Use lowercase letters.".into(),
                elements: vec![
                    element("e1", "textbox", "Address", clicks.is_multiple_of(2)),
                    element("e2", "button", "Continue", !clicks.is_multiple_of(2)),
                ],
                revision: clicks as u64,
                notices: vec![],
                ..Observation::default()
            })
        }

        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn act(&self, _: &str, _: &Action) -> ViewResult<()> {
            *self.clicks.lock().unwrap() += 1;
            Ok(())
        }
    }

    /// Clicks Continue, whatever it shows.
    struct Stubborn;

    fn sure(choice: &str) -> Answer {
        Answer::Choice {
            choice: choice.into(),
            probabilities: [(choice.to_string(), 1.0)].into(),
            confidence: 1.0,
        }
    }

    impl Decider for Stubborn {
        fn decide(
            &self,
            _: &Value,
            _: &BTreeMap<String, Question>,
        ) -> Result<Decision, DecideError> {
            Ok(Decision {
                answers: [
                    ("operation".to_string(), sure("CLICK")),
                    ("click_target".to_string(), sure("e2")),
                ]
                .into(),
                ..Decision::default()
            })
        }
    }

    #[test]
    fn the_same_action_that_changes_nothing_one_reads_ends_the_run_on_its_third_try() {
        let form = Refusing::default();
        let run = GoalRun {
            view: &form,
            decider: &Stubborn,
            on_step: &|_| {},
            pause: &pause,
            max_actions: 20,
        };
        let outcome = run.resume("continue", &[], vec![]);
        assert_eq!(outcome.stop, Stop::Repeated, "{}", outcome.stop);
        assert_eq!(*form.clicks.lock().unwrap(), 3);
    }
}
