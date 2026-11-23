//! A link Command-clicked in a terminal: asked where it opens unless the person chose one place,
//! opened where their answer says, and never followed when it is no web page's address.

use super::{Clicked, Place, answered, clicked, question};
use crate::browser::dialog::{Answer, Dialog};
use crate::browser::settings::OpenLinks;

#[test]
fn a_web_link_asks_by_default_and_opens_at_once_where_the_person_chose() {
    for address in [
        "https://example.com/a?b=1",
        "http://localhost:3000",
        "HTTPS://Example.com",
    ] {
        assert_eq!(
            clicked(OpenLinks::Ask, address),
            Some(Clicked::Ask),
            "{address}"
        );
        assert_eq!(
            clicked(OpenLinks::Tab, address),
            Some(Clicked::Open(Place::Tab))
        );
        assert_eq!(
            clicked(OpenLinks::Browser, address),
            Some(Clicked::Open(Place::Browser))
        );
    }
}

#[test]
fn a_link_that_is_no_web_address_is_never_followed_nor_handed_to_another_app() {
    for address in [
        "file:///Applications/Calculator.app",
        "mailto:someone@example.com",
        "javascript:alert(1)",
        "ssh://host",
        "someapp://open?x",
        "http:/x",
        "",
    ] {
        for setting in [OpenLinks::Ask, OpenLinks::Tab, OpenLinks::Browser] {
            assert_eq!(
                clicked(setting, address),
                None,
                "{address} with {setting:?}"
            );
        }
    }
}

#[test]
fn the_question_names_the_address_and_its_answer_says_where_it_opens() {
    let dialog = Dialog::question(&question("https://example.com/a"));
    assert_eq!(dialog.title, "Open Link?");
    assert_eq!(dialog.detail, "https://example.com/a");
    assert_eq!(
        (
            dialog.yes.as_str(),
            dialog.other.as_deref(),
            dialog.no.as_str()
        ),
        ("Open in Tab", Some("Open in Browser"), "Cancel")
    );
    assert!(
        dialog.asked,
        "the person's own click: it takes the keys, Return opens the tab"
    );
    assert_eq!(answered(Answer::Yes), Some(Place::Tab));
    assert_eq!(answered(Answer::Other), Some(Place::Browser));
    assert_eq!(answered(Answer::No), None, "Cancel opens nothing");
}
