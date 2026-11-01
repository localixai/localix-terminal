//! Tests of `ask`.

use super::*;
use crate::view::SelectOption;

fn element(reference: &str, role: &str, name: &str) -> Element {
    Element {
        reference: reference.into(),
        role: role.into(),
        name: name.into(),
        value: None,
        states: vec![],
        editable: role == "textbox",
        dialog: false,
        options: vec![],
        form: None,
    }
}

fn view(elements: Vec<Element>) -> Observation {
    Observation {
        outline: String::new(),
        epoch: "1".into(),
        location: "http://x".into(),
        title: "T".into(),
        text: "Book a trip".into(),
        scroll: 0,
        elements,
        busy: false,
        revision: 0,
        notices: vec![],
    }
}

fn choice(choice: &str) -> Answer {
    Answer::Choice {
        choice: choice.into(),
        probabilities: [(choice.to_string(), 1.0)].into(),
        confidence: 1.0,
    }
}

#[test]
fn offers_only_what_each_operation_can_act_on() {
    let mut disabled = element("e3", "button", "Later");
    disabled.states.push("disabled".into());
    let mut class = element("e4", "combobox", "Class");
    class.options = vec![SelectOption {
        value: "b".into(),
        label: "Business".into(),
    }];
    let view = view(vec![
        element("e1", "textbox", "City"),
        element("e2", "button", "Search"),
        disabled,
        class,
    ]);
    let values = vec!["Zurich".to_string()];
    let ask = ask(&AskInput {
        goal: "g",
        conventions: "",
        abilities: &[],
        view: &view,
        values: &values,
        history: &[],
        paid_before: &[],
    });
    let keys = |head: &str| ask.offered[head].keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys("click_target"), ["e1", "e2"]);
    assert_eq!(keys("type_entry"), ["e1=v1"]);
    assert_eq!(keys("select_target"), ["e4:1"]);
    assert!(
        !ask.offered["operation"].contains_key("PRESS_ENTER"),
        "Enter only right after typing"
    );

    let answers = [
        ("operation".into(), choice("TYPE_TEXT")),
        ("type_entry".into(), choice("e1=v1")),
    ]
    .into();
    let pick = ask.read(&answers).unwrap();
    assert_eq!(
        (pick.reference.as_deref(), pick.value.as_deref()),
        (Some("e1"), Some("Zurich"))
    );
    let answers = [
        ("operation".into(), choice("SELECT")),
        ("select_target".into(), choice("e4:1")),
    ]
    .into();
    let pick = ask.read(&answers).unwrap();
    assert_eq!(
        (pick.reference.as_deref(), pick.value.as_deref()),
        (Some("e4"), Some("b"))
    );
    let answers = [
        ("operation".into(), choice("CLICK")),
        ("click_target".into(), choice("e3")),
    ]
    .into();
    assert!(
        ask.read(&answers).is_err(),
        "a disabled control is never a target"
    );
}

#[test]
fn typing_offers_whole_field_value_pairs_and_skips_what_is_already_there() {
    let mut from = element("e1", "textbox", "Where from?");
    from.value = Some("Larnaca".into());
    let view = view(vec![from, element("e2", "textbox", "Where to?")]);
    let values = vec!["Larnaca".to_string(), "Tbilisi".to_string()];
    let ask = ask(&AskInput {
        goal: "g",
        conventions: "",
        abilities: &[],
        view: &view,
        values: &values,
        history: &[],
        paid_before: &[],
    });
    let offered: Vec<&Value> = ask.offered["type_entry"].values().collect();
    // "Where from?" holds a value: filled. "Larnaca" is in a field: placed. Neither is offered.
    assert_eq!(offered, [&json!("textbox \"Where to?\" ← \"Tbilisi\"")]);
    let answers = [
        ("operation".into(), choice("TYPE_TEXT")),
        ("type_entry".into(), choice("e2=v2")),
    ]
    .into();
    let pick = ask.read(&answers).unwrap();
    assert_eq!(
        (pick.reference.as_deref(), pick.value.as_deref()),
        (Some("e2"), Some("Tbilisi"))
    );
}

#[test]
fn a_dialog_scopes_the_offer_and_scrolling_stops_when_it_stops_moving() {
    let mut accept = element("e9", "button", "Accept");
    accept.dialog = true;
    let view = view(vec![element("e1", "button", "Search"), accept]);
    let still = GoalStep {
        operation: Operation::ScrollDown,
        reference: None,
        element: None,
        value: None,
        probability: 1.0,
        changed: Some(false),
        failed: None,
        new_view: false,
        control: None,
        commit: None,
    };
    let ask = ask(&AskInput {
        goal: "g",
        conventions: "",
        abilities: &[],
        view: &view,
        values: &[],
        history: std::slice::from_ref(&still),
        paid_before: &[],
    });
    assert_eq!(
        ask.offered["click_target"].keys().collect::<Vec<_>>(),
        ["e9"]
    );
    assert_eq!(ask.state["dialog_open"], json!(true));
    assert!(!ask.offered["operation"].contains_key("SCROLL_DOWN"));
    assert!(ask.offered["operation"].contains_key("SCROLL_UP"));
    let idle = GoalStep {
        operation: Operation::Wait,
        ..still
    };
    let ask = super::ask(&AskInput {
        goal: "g",
        conventions: "",
        abilities: &[],
        view: &view,
        values: &[],
        history: &[idle],
        paid_before: &[],
    });
    assert!(
        !ask.offered["operation"].contains_key("WAIT"),
        "a stable view is not waited on again"
    );
}

#[test]
fn a_settled_view_offers_no_wait_after_a_step_that_showed_something() {
    let next = GoalStep {
        operation: Operation::Click,
        reference: Some("e1".into()),
        element: Some("button \"Next\"".into()),
        value: None,
        probability: 1.0,
        changed: Some(true),
        failed: None,
        new_view: true,
        control: None,
        commit: None,
    };
    let offered = |history: &[GoalStep], busy: bool| {
        let mut view = view(vec![element("e1", "button", "Next")]);
        view.busy = busy;
        let ask = ask(&AskInput {
            goal: "Click Next and wait for the next question",
            conventions: "",
            abilities: &[],
            view: &view,
            values: &[],
            history,
            paid_before: &[],
        });
        ask.offered["operation"].contains_key("WAIT")
    };
    assert!(
        !offered(std::slice::from_ref(&next), false),
        "the next question is there"
    );
    assert!(
        offered(std::slice::from_ref(&next), true),
        "the view says it is working"
    );
    let silent = GoalStep {
        changed: Some(false),
        new_view: false,
        control: None,
        commit: None,
        ..next
    };
    assert!(
        offered(&[silent], false),
        "a click that has shown nothing yet"
    );
}

#[test]
fn the_model_chooses_how_long_to_wait() {
    let mut view = view(vec![element("e1", "button", "Search")]);
    view.busy = true;
    let ask = ask(&AskInput {
        goal: "Search and wait for the results",
        conventions: "",
        abilities: &[],
        view: &view,
        values: &[],
        history: &[],
        paid_before: &[],
    });
    assert!(ask.offered["wait_for"].contains_key("10"));
    let answers = [
        ("operation".into(), choice("WAIT")),
        ("wait_for".into(), choice("10")),
    ]
    .into();
    let pick = ask.read(&answers).unwrap();
    assert_eq!(pick.operation, Operation::Wait);
    assert_eq!(pick.value.as_deref(), Some("10"));
}

#[test]
fn a_target_refused_as_covered_is_not_offered_again_until_the_view_changes() {
    let view = view(vec![
        element("e1", "button", "New Game"),
        element("e2", "button", "Save"),
    ]);
    let refused = GoalStep {
        operation: Operation::Click,
        reference: Some("e1".into()),
        element: None,
        value: None,
        probability: 0.9,
        changed: Some(false),
        failed: Some("covered: a dialog \"Consent\" is on top; answer or close it first".into()),
        new_view: false,
        control: None,
        commit: None,
    };
    let offered = |history: &[GoalStep]| {
        let ask = ask(&AskInput {
            goal: "g",
            conventions: "",
            abilities: &[],
            view: &view,
            values: &[],
            history,
            paid_before: &[],
        });
        ask.offered["click_target"]
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(offered(std::slice::from_ref(&refused)), ["e2"]);
    let moved = GoalStep {
        reference: Some("e2".into()),
        changed: Some(true),
        failed: None,
        ..refused.clone()
    };
    assert_eq!(
        offered(&[refused, moved]),
        ["e1", "e2"],
        "the view changed since"
    );
}

#[test]
fn the_operation_is_chosen_knowing_the_values_it_may_type() {
    // A goal that says "type the given expression" names no text: without the values the model
    // cannot tell that typing would progress, and calls it blocked.
    let mut terminal = view(vec![element("input", "textbox", "terminal input")]);
    terminal.outline = ">>>".into();
    let values = vec!["7 + 7".to_string()];
    let asked = |values: &[String]| {
        ask(&AskInput {
            goal: "Type the given expression at the prompt and press Enter",
            conventions: "",
            abilities: &[],
            view: &terminal,
            values,
            history: &[],
            paid_before: &[],
        })
        .state
    };
    assert_eq!(asked(&values)["values"], json!(["7 + 7"]));
    assert!(asked(&[]).get("values").is_none(), "no values, no key");
    // A view read as its text and controls is told them the same way.
    let form = view(vec![element("e1", "textbox", "City")]);
    let state = ask(&AskInput {
        goal: "g",
        conventions: "",
        abilities: &[],
        view: &form,
        values: &values,
        history: &[],
        paid_before: &[],
    })
    .state;
    assert_eq!(state["values"], json!(["7 + 7"]));
}

#[test]
fn an_option_already_chosen_is_never_offered_again() {
    let option = |value: &str, label: &str| SelectOption {
        value: value.into(),
        label: label.into(),
    };
    let mut seat = element("e3", "combobox", "Seat class");
    seat.value = Some("Economy".into());
    seat.options = vec![option("eco", "Economy"), option("biz", "Business")];
    let mut only = element("e2", "combobox", "Currency");
    only.value = Some("EUR".into());
    only.options = vec![option("eur", "EUR")];
    let asked = ask_of(&view(vec![seat, element("e1", "button", "Language")]));
    let keys: Vec<String> = asked.offered["select_target"].keys().cloned().collect();
    assert_eq!(keys, ["e3:2"]);
    // A dropdown with no other option to choose offers no SELECT at all.
    let asked = ask_of(&view(vec![only, element("e1", "button", "Language")]));
    assert!(!asked.offered["operation"].contains_key("SELECT"));
}

fn ask_of(view: &Observation) -> Ask {
    ask(&AskInput {
        goal: "g",
        conventions: "",
        abilities: &[],
        view,
        values: &[],
        history: &[],
        paid_before: &[],
    })
}

#[test]
fn a_view_that_takes_keys_is_offered_every_shared_key_and_the_pick_names_it() {
    let view = view(vec![element("e1", "button", "New game")]);
    let input = |abilities: &'static [Ability]| AskInput {
        goal: "play",
        conventions: "",
        abilities,
        view: &view,
        values: &[],
        history: &[],
        paid_before: &[],
    };
    let without = ask(&input(&[]));
    assert!(!without.questions.contains_key(KEY_HEAD));
    let with = ask(&input(&[Ability::Keys]));
    let Question::Choice { criteria, .. } = &with.questions[KEY_HEAD] else {
        panic!("no key question")
    };
    let offered: Vec<&str> = criteria.keys().map(String::as_str).collect();
    let mut shared = crate::view::KEYS.to_vec();
    shared.sort_unstable();
    assert_eq!(offered, shared);
    let answers = [
        ("operation".to_string(), choice("PRESS_KEY")),
        (KEY_HEAD.to_string(), choice("ArrowLeft")),
    ]
    .into();
    let pick = with.read(&answers).unwrap();
    assert_eq!(pick.operation, Operation::PressKey);
    assert_eq!(pick.value.as_deref(), Some("ArrowLeft"));
}

#[test]
fn only_supplied_addresses_are_offered_and_only_where_the_view_takes_one() {
    let view = view(vec![element("e1", "textbox", "Name")]);
    let values = vec!["Robin".to_string(), "https://example.com/a".to_string()];
    let input = |abilities: &'static [Ability]| AskInput {
        goal: "go",
        conventions: "",
        abilities,
        view: &view,
        values: &values,
        history: &[],
        paid_before: &[],
    };
    assert!(
        !ask(&input(&[Ability::Keys]))
            .questions
            .contains_key(ADDRESS_HEAD)
    );
    let with = ask(&input(&[Ability::Address]));
    let Question::Choice { criteria, .. } = &with.questions[ADDRESS_HEAD] else {
        panic!("no address question")
    };
    assert_eq!(criteria.len(), 1, "{criteria:?}");
    let answers = [
        ("operation".to_string(), choice("GO_TO")),
        (ADDRESS_HEAD.to_string(), choice("a1")),
    ]
    .into();
    let pick = with.read(&answers).unwrap();
    assert_eq!(pick.operation, Operation::GoTo);
    assert_eq!(pick.value.as_deref(), Some("https://example.com/a"));
}

#[test]
fn what_the_view_told_and_what_is_held_back_reach_the_model() {
    let mut told = view(vec![
        element("e1", "button", "Pay now"),
        element("e2", "button", "Back"),
    ]);
    told.notices = vec!["a confirm: \"Sure?\", answered Cancel".into()];
    let ask = ask(&AskInput {
        goal: "go back",
        conventions: "",
        abilities: &[],
        view: &told,
        values: &[],
        history: &[],
        paid_before: &[],
    });
    assert_eq!(
        ask.state["told"][0],
        "a confirm: \"Sure?\", answered Cancel"
    );
    assert_eq!(
        ask.state["withheld"][0],
        "button \"Pay now\": the goal does not ask to pay or order"
    );
    assert_eq!(
        ask.offered["click_target"].keys().collect::<Vec<_>>(),
        ["e2"]
    );
}

#[test]
fn a_password_field_holds_what_was_typed_into_it_though_it_reads_as_dots() {
    let mut password = element("e2", "textbox", "Password");
    password.value = Some("••••••".into());
    let login = view(vec![element("e1", "textbox", "Email"), password.clone()]);
    password.value = None;
    let typed = GoalStep {
        operation: Operation::TypeText,
        reference: Some("e2".into()),
        element: Some(password.describe()),
        value: Some("s3cret!".into()),
        probability: 1.0,
        changed: Some(true),
        failed: None,
        new_view: false,
        control: Some(password.label()),
        commit: None,
    };
    let values = ["ada@example.com".to_string(), "s3cret!".to_string()];
    let ask = ask(&AskInput {
        goal: "log in",
        conventions: "",
        abilities: &[],
        view: &login,
        values: &values,
        history: std::slice::from_ref(&typed),
        paid_before: &[],
    });
    assert_eq!(
        ask.offered["type_entry"].keys().collect::<Vec<_>>(),
        ["e1=v1"],
        "the password is typed, and the e-mail goes into its own field only"
    );
}

#[test]
fn a_field_the_page_refused_is_offered_the_other_values_not_the_refused_one() {
    let mut address = element("e1", "textbox", "Workspace address");
    address.value = Some("Pine Crew".into());
    let mut name = element("e2", "textbox", "Team name");
    name.value = Some("Pine Crew".into());
    let values = ["Pine Crew".to_string(), "pine-crew".to_string()];
    let offered = |address: &Element| {
        let page = view(vec![address.clone(), name.clone()]);
        let ask = ask(&AskInput {
            goal: "use the address pine-crew if Pine Crew is refused",
            conventions: "",
            abilities: &[],
            view: &page,
            values: &values,
            history: &[],
            paid_before: &[],
        });
        (ask.offered.get("type_entry").into_iter())
            .flat_map(|entries| entries.keys().cloned())
            .collect::<Vec<_>>()
    };
    assert!(
        offered(&address).is_empty(),
        "a filled form trades no values"
    );
    address.states = vec!["invalid: Use 3 to 24 lowercase letters".into()];
    assert_eq!(offered(&address), ["e1=v2"]);
}

#[test]
fn enter_in_a_field_of_a_paying_form_is_held_back_unless_the_goal_asks_to_pay() {
    let mut card = element("e1", "textbox", "Card number");
    card.form = Some("f1".into());
    card.value = Some("4242".into());
    let mut place = element("e2", "button", "Place order");
    place.form = Some("f1".into());
    let mut search = element("e3", "textbox", "Search");
    search.form = Some("f2".into());
    let page = view(vec![card.clone(), place, search]);
    let typed = GoalStep {
        operation: Operation::TypeText,
        reference: Some("e1".into()),
        element: Some(card.describe()),
        value: Some("4242".into()),
        probability: 1.0,
        changed: Some(true),
        failed: None,
        new_view: false,
        control: Some(card.label()),
        commit: None,
    };
    let values = ["4242".to_string()];
    let asked = |goal: &str| {
        ask(&AskInput {
            goal,
            conventions: "",
            abilities: &[Ability::Keys],
            view: &page,
            values: &values,
            history: std::slice::from_ref(&typed),
            paid_before: &[],
        })
    };
    let fill = asked("fill in the card number");
    assert!(!fill.offered["operation"].contains_key("PRESS_ENTER"));
    assert!(!fill.offered[KEY_HEAD].contains_key("Enter"));
    assert!(fill.offered[KEY_HEAD].contains_key("Tab"));
    assert!(
        fill.withheld
            .iter()
            .any(|why| why.starts_with("Enter, as it sends the form of button \"Place order\"")),
        "{:?}",
        fill.withheld
    );
    let pay = asked("fill in the card number and place the order");
    assert!(pay.offered["operation"].contains_key("PRESS_ENTER"));
    assert!(pay.offered[KEY_HEAD].contains_key("Enter"));
}
