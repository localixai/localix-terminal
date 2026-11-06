//! Tests of the tab menu's items: what each kind of tab offers.

use super::{Around, Item, groups};
use crate::browser::strip::Key;

#[test]
fn a_web_page_can_go_to_the_persons_browser_and_its_address_be_copied() {
    let menu = groups(
        Key::Web(1),
        "https://example.com/",
        Around {
            folder: true,
            others: true,
            right: false,
            split: None,
        },
    );
    assert_eq!(
        menu,
        [
            vec![Item::Reload, Item::Duplicate],
            vec![Item::Outside, Item::CopyAddress],
            vec![Item::Reveal],
            vec![Item::Close, Item::CloseOthers],
        ]
    );
}

#[test]
fn a_page_with_no_web_address_has_nothing_to_open_elsewhere() {
    let menu = groups(Key::Web(1), "about:blank", Around::default());
    assert_eq!(
        menu,
        [vec![Item::Reload, Item::Duplicate], vec![Item::Close]]
    );
}

#[test]
fn a_terminal_or_the_board_offers_its_projects_folder_and_a_terminal_in_it() {
    for key in [Key::Term(2), Key::Term(crate::browser::BOARD + 1)] {
        let menu = groups(
            key,
            "",
            Around {
                folder: true,
                others: false,
                right: true,
                split: None,
            },
        );
        assert_eq!(
            menu,
            [
                vec![Item::NewTerminal, Item::Reveal],
                vec![Item::Close, Item::CloseRight],
            ]
        );
    }
}

#[test]
fn a_tab_goes_beside_the_one_shown_and_a_half_makes_the_window_whole() {
    let beside = groups(
        Key::Term(2),
        "",
        Around {
            split: Some(false),
            ..Around::default()
        },
    );
    assert!(beside.contains(&vec![Item::Split]), "{beside:?}");
    let half = groups(
        Key::Web(1),
        "about:blank",
        Around {
            split: Some(true),
            ..Around::default()
        },
    );
    assert!(half.contains(&vec![Item::Unsplit]), "{half:?}");
}
