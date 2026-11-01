//! Tests of `commit`.

use super::{Commit, held_back};
use crate::goal::step::{GoalStep, Operation};
use crate::view::Element;

fn control(role: &str, name: &str) -> Element {
    Element {
        reference: "e1".into(),
        role: role.into(),
        name: name.into(),
        value: None,
        states: vec![],
        editable: false,
        dialog: false,
        options: vec![],
        form: None,
    }
}

fn clicked(element: &Element, changed: bool) -> GoalStep {
    GoalStep {
        operation: Operation::Click,
        reference: Some(element.reference.clone()),
        element: Some(element.describe()),
        value: None,
        probability: 1.0,
        changed: Some(changed),
        failed: None,
        new_view: changed,
        control: Some(element.label()),
        commit: Commit::of(element),
    }
}

#[test]
fn a_control_commits_by_the_whole_words_of_its_name() {
    let of = |role, name| Commit::of(&control(role, name));
    assert_eq!(of("button", "Pay $20"), Some(Commit::Pays));
    assert_eq!(of("button", "Place order"), Some(Commit::Pays));
    assert_eq!(of("link", "Book now"), Some(Commit::Pays));
    assert_eq!(of("button", "Delete row"), Some(Commit::Deletes));
    assert_eq!(of("button", "Send"), Some(Commit::Sends));
    assert_eq!(of("button", "Sign up"), Some(Commit::Sends));
    assert_eq!(of("link", "Books"), None, "a word inside another is not it");
    assert_eq!(of("link", "Payment methods"), None);
    // A long name says what it acts on after its verb: an item's words are not the action.
    assert_eq!(of("button", "Edit Buy oat milk and coffee beans"), None);
    assert_eq!(
        of("button", "Delete Book the dentist for March"),
        Some(Commit::Deletes)
    );
    assert_eq!(of("button", "Add Monstera to cart"), None);
    assert_eq!(of("button", "Proceed to checkout"), Some(Commit::Pays));
    assert_eq!(of("button", "Confirm and pay $54.20"), Some(Commit::Pays));
    // A promo code or a filter is applied far more often than an application is sent.
    assert_eq!(of("button", "Apply"), None);
    assert_eq!(of("button", "Next"), None);
    assert_eq!(
        of("textbox", "Pay"),
        None,
        "a field is typed into, not clicked"
    );
}

#[test]
fn paying_and_deleting_are_held_back_unless_the_goal_asks_and_sending_never_is() {
    let pay = control("button", "Pay $20");
    let delete = control("button", "Delete");
    let send = control("button", "Send");
    assert!(held_back(&pay, "open the settings", &[], &[]).is_some());
    assert!(held_back(&pay, "buy the blue plant", &[], &[]).is_none());
    assert!(held_back(&pay, "complete the purchase", &[], &[]).is_none());
    assert!(held_back(&delete, "tidy the list", &[], &[]).is_some());
    assert!(held_back(&delete, "remove the second todo", &[], &[]).is_none());
    assert!(held_back(&send, "say hello", &[], &[]).is_none());
    assert_eq!(
        held_back(&pay, "open the settings", &[], &[]).as_deref(),
        Some("button \"Pay $20\": the goal does not ask to pay or order")
    );
}

#[test]
fn a_paying_control_that_acted_once_is_held_back_but_one_that_changed_nothing_is_not() {
    let pay = control("button", "Place order");
    let goal = "place the order";
    assert!(held_back(&pay, goal, &[clicked(&pay, false)], &[]).is_none());
    assert_eq!(
        held_back(&pay, goal, &[clicked(&pay, true)], &[]).as_deref(),
        Some("button \"Place order\": this goal clicked it once already")
    );
    let delete = control("button", "Delete");
    assert!(
        held_back(
            &delete,
            "delete the done ones",
            &[clicked(&delete, true)],
            &[]
        )
        .is_none()
    );
}

#[test]
fn how_to_tell_a_goal_is_done_asks_for_nothing() {
    let pay = control("button", "Check out");
    assert!(
        held_back(
            &pay,
            "done when the cart shows the order was placed",
            &[],
            &[]
        )
        .is_some()
    );
    assert!(
        held_back(
            &pay,
            "open the cart; done when the order total shows",
            &[],
            &[]
        )
        .is_some()
    );
    assert!(held_back(&pay, "place the order; done when it is placed", &[], &[]).is_none());
}

#[test]
fn a_paying_control_an_earlier_goal_clicked_is_clicked_again_only_when_asked_again() {
    let pay = control("button", "Check out");
    let earlier = [pay.label()];
    let why = held_back(
        &pay,
        "check out; done when the order is placed",
        &[],
        &earlier,
    )
    .unwrap();
    assert!(
        why.contains("an earlier goal in this tab clicked it already"),
        "{why}"
    );
    assert!(
        held_back(
            &pay,
            "place another order; done when it is placed",
            &[],
            &earlier
        )
        .is_none()
    );
    assert!(held_back(&pay, "check out again", &[], &earlier).is_none());
    let other = control("button", "Pay $20");
    assert!(held_back(&other, "pay; done when paid", &[], &earlier).is_none());
}

#[test]
fn a_goal_that_says_not_to_pay_or_delete_does_not_ask_to() {
    let book = control("button", "Book now");
    let delete = control("button", "Delete");
    let held = |c: &Element, goal: &str| held_back(c, goal, &[], &[]).is_some();
    assert!(held(
        &book,
        "pick the dates; done when the price shows, without booking"
    ));
    assert!(held(&book, "show the price but do not book"));
    assert!(held(&book, "show the price, don't book the hut"));
    assert!(held(&book, "never pay for anything"));
    assert!(held(&delete, "rename it, not delete it"));
    assert!(!held(&book, "book the hut, not the cabin"));
    assert!(!held(&book, "do not wait: book the hut"));
}

#[test]
fn enter_sends_the_paying_control_of_the_typed_fields_form_and_none_outside_a_form() {
    let mut card = control("textbox", "Card number");
    card.reference = "e1".into();
    card.form = Some("f1".into());
    let mut save = control("button", "Save card");
    save.reference = "e2".into();
    save.form = Some("f1".into());
    let mut pay = control("button", "Pay $20");
    pay.reference = "e3".into();
    pay.form = Some("f1".into());
    let mut elsewhere = control("button", "Delete account");
    elsewhere.reference = "e4".into();
    let typed = GoalStep {
        operation: Operation::TypeText,
        commit: None,
        ..clicked(&card, true)
    };
    let elements = [card.clone(), save, pay, elsewhere];
    let sent = super::sent_by_enter(&elements, std::slice::from_ref(&typed));
    assert_eq!(
        sent.map(Element::label).as_deref(),
        Some("button \"Pay $20\"")
    );
    let mut alone = card;
    alone.form = None;
    assert!(super::sent_by_enter(&[alone], &[typed]).is_none());
}
