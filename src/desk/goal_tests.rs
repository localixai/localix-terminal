//! Tests of `goal`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use serde_json::Value;

use super::*;
use crate::model::{Answer, DecideError, Decision, Question};
use crate::view::{Action, Element, Observation, ViewResult};

/// One button, by its name; once it is clicked the view says so.
struct Door(Mutex<bool>, &'static str);

impl View for Door {
    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn observe(&self) -> ViewResult<Observation> {
        let open = *self.0.lock().unwrap();
        Ok(Observation {
            epoch: open.to_string(),
            location: "door".into(),
            title: "Door".into(),
            text: if open { "open" } else { "closed" }.into(),
            elements: vec![Element {
                reference: "e1".into(),
                role: "button".into(),
                name: self.1.into(),
                value: None,
                states: vec![],
                editable: false,
                dialog: false,
                options: vec![],
                form: None,
            }],
            ..Observation::default()
        })
    }

    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn act(&self, _: &str, _: &Action) -> ViewResult<()> {
        *self.0.lock().unwrap() = true;
        Ok(())
    }
}

/// Clicks the button, then says it is done.
struct Opener;

fn sure(choice: &str) -> Answer {
    Answer::Choice {
        choice: choice.into(),
        probabilities: [(choice.to_string(), 1.0)].into(),
        confidence: 1.0,
    }
}

impl Decider for Opener {
    fn decide(
        &self,
        state: &Value,
        _: &BTreeMap<String, Question>,
    ) -> Result<Decision, DecideError> {
        let answers = if state["view"]["text"] == "open" {
            [("operation".to_string(), sure("DONE"))].into()
        } else {
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
fn each_step_of_a_goal_is_told_to_the_call_with_the_tab_it_acts_on() {
    let told = Mutex::new(Vec::new());
    let progress = |tab: &str, step: &str| {
        told.lock()
            .unwrap()
            .push((tab.to_string(), step.to_string()))
    };
    let call = Call {
        owner: "me",
        cwd: Path::new("/tmp"),
        vars: None,
        key: None,
        progress: Some(&progress),
    };
    let report = drive(
        &call,
        "w3",
        &Door(Mutex::new(false), "Open"),
        &Opener,
        "open the door",
        &[],
        &mut vec![],
    )
    .unwrap();
    assert!(report.contains("done"), "{report}");
    let told = told.into_inner().unwrap();
    assert!(told.len() >= 2, "{told:?}");
    assert!(told.iter().all(|(tab, _)| tab == "w3"));
    assert!(told[0].1.starts_with("CLICK"), "{told:?}");
    assert!(told.last().unwrap().1.starts_with("DONE"), "{told:?}");
}

fn quiet_call() -> Call<'static> {
    Call {
        owner: "me",
        cwd: Path::new("/tmp"),
        vars: None,
        key: None,
        progress: None,
    }
}

#[test]
fn a_goal_that_does_not_ask_to_pay_never_clicks_pay_and_says_it_held_it_back() {
    let door = Door(Mutex::new(false), "Pay $20");
    let report = drive(
        &quiet_call(),
        "w1",
        &door,
        &Opener,
        "open the door",
        &[],
        &mut vec![],
    )
    .unwrap();
    assert!(!*door.0.lock().unwrap(), "it paid: {report}");
    assert!(
        report.contains(
            "Held back from it, as the goal does not ask to pay or order: button \"Pay $20\"."
        ),
        "{report}"
    );
}

#[test]
fn a_goal_that_asks_to_pay_pays_and_the_report_says_it_may_have() {
    let door = Door(Mutex::new(false), "Pay $20");
    let report = drive(
        &quiet_call(),
        "w1",
        &door,
        &Opener,
        "pay for the door",
        &[],
        &mut vec![],
    )
    .unwrap();
    assert!(*door.0.lock().unwrap(), "{report}");
    assert!(
        report.contains(
            "It clicked what commits: step 1, button \"Pay $20\": may have paid or ordered"
        ),
        "{report}"
    );
}

#[test]
fn a_later_goal_in_the_tab_does_not_pay_again_unless_it_asks_to_do_it_again() {
    let door = Door(Mutex::new(false), "Pay $20");
    let mut paid = vec![];
    let run = |goal: &str, paid: &mut Vec<String>| {
        let view = Sealed {
            view: &door,
            secrets: vec![],
            paid: paid.clone(),
        };
        drive(&quiet_call(), "w1", &view, &Opener, goal, &[], paid).unwrap()
    };
    run("pay for the door", &mut paid);
    assert_eq!(paid, ["button \"Pay $20\""]);

    // The page did not change back (a cart kept after an order): a second goal finds it again.
    *door.0.lock().unwrap() = false;
    let report = run("pay for the door; done when it opens", &mut paid);
    assert!(!*door.0.lock().unwrap(), "it paid twice: {report}");
    assert!(
        report.contains("an earlier goal in this tab clicked it already"),
        "{report}"
    );
    let report = run("pay for the door again", &mut paid);
    assert!(*door.0.lock().unwrap(), "{report}");
}

#[test]
fn controls_held_back_for_one_reason_are_one_line_the_many_counted() {
    let why = "the goal does not ask to delete";
    let withheld: Vec<String> = (1..=5)
        .map(|i| format!("button \"Delete {i}\": {why}"))
        .collect();
    let mut withheld = withheld;
    withheld.push("button \"Pay\": this goal clicked it once already".into());
    assert_eq!(
        held_back(&withheld),
        [
            "Held back from it, as the goal does not ask to delete: button \"Delete 1\", button \"Delete 2\", button \"Delete 3\" and 2 more.",
            "Held back from it, as this goal clicked it once already: button \"Pay\".",
        ]
    );
}
