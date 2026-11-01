//! Tests of `Sealed`.

use std::sync::Mutex;

use super::Sealed;
use crate::view::{Action, Element, Observation, View, ViewResult};

/// A field that shows what was typed into it, on a page that shows a key.
struct Field(Mutex<String>);

impl View for Field {
    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn observe(&self) -> ViewResult<Observation> {
        let typed = self.0.lock().unwrap().clone();
        Ok(Observation {
            text: format!("Your key: sk-proj-4f9aXbQ2kLm8Zr7Tq1Wv. Typed {typed}"),
            elements: vec![Element {
                reference: "e1".into(),
                role: "textbox".into(),
                name: "Password".into(),
                value: Some(typed),
                states: vec![],
                editable: true,
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
    fn act(&self, _: &str, action: &Action) -> ViewResult<()> {
        if let Action::Type { text, .. } = action {
            *self.0.lock().unwrap() = text.clone();
        }
        Ok(())
    }
}

#[test]
fn a_secret_is_typed_whole_and_read_back_as_its_reference_and_a_key_is_never_shown() {
    let field = Field(Mutex::new(String::new()));
    let sealed = Sealed {
        view: &field,
        secrets: vec![("PW".into(), "hunter2!".into())],
        paid: vec![],
    };
    let typed = Action::Type {
        reference: "e1".into(),
        text: "{{secret:PW}}".into(),
    };
    sealed.act("", &typed).unwrap();
    assert_eq!(
        *field.0.lock().unwrap(),
        "hunter2!",
        "the page got the secret"
    );
    let seen = sealed.observe().unwrap();
    assert_eq!(seen.elements[0].value.as_deref(), Some("{{secret:PW}}"));
    assert_eq!(seen.text, "Your key: [credential]. Typed {{secret:PW}}");
}

#[test]
fn a_secret_the_goal_does_not_name_is_masked_too_in_a_fields_state_and_an_options_label() {
    struct Shown;
    impl View for Shown {
        fn observe(&self) -> ViewResult<Observation> {
            Ok(Observation {
                text: "echo db-pass-9".into(),
                elements: vec![Element {
                    reference: "e1".into(),
                    role: "combobox".into(),
                    name: "Account".into(),
                    value: None,
                    states: vec!["invalid: db-pass-9 is wrong".into()],
                    editable: false,
                    dialog: false,
                    options: vec![crate::view::SelectOption {
                        value: "v1".into(),
                        label: "db-pass-9".into(),
                    }],
                    form: None,
                }],
                ..Observation::default()
            })
        }

        fn act(&self, _: &str, _: &Action) -> ViewResult<()> {
            Ok(())
        }
    }
    let sealed = Sealed {
        view: &Shown,
        secrets: vec![("DB".into(), "db-pass-9".into())],
        paid: vec![],
    };
    let seen = sealed.observe().unwrap();
    assert_eq!(seen.text, "echo {{secret:DB}}");
    assert_eq!(seen.elements[0].states, ["invalid: {{secret:DB}} is wrong"]);
    assert_eq!(seen.elements[0].options[0].label, "{{secret:DB}}");
    assert_eq!(
        seen.elements[0].options[0].value, "v1",
        "what is selected stays"
    );
}
