//! Tests of the tab menu's items: what each kind of tab offers.

use super::{Around, Item, beside, groups, own_file};
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

#[test]
fn a_documents_other_tabs_are_the_documents_of_its_row() {
    // Its session's terminal and the other sessions are no tabs of its row: closing "the others"
    // from a file never closes a terminal.
    let rows = [Key::Term(1), Key::Term(2)];
    let stack = [
        Key::Web(1),
        Key::Term(crate::browser::FILE + 1),
        Key::Web(2),
    ];
    let (others, right) = beside(stack[1], Some(&stack), &rows);
    assert_eq!(others, [Key::Web(1), Key::Web(2)]);
    assert_eq!(right, [Key::Web(2)]);
    let (others, right) = beside(stack[2], Some(&stack), &rows);
    assert_eq!(others, [stack[0], stack[1]]);
    assert!(right.is_empty());
}

#[test]
fn a_sessions_other_tabs_are_the_rows_of_the_side() {
    // A session's documents close with it and are no tabs beside it: "the others" from a
    // terminal leaves its own files and pages open.
    let rows = [Key::Term(1), Key::Term(2), Key::Term(3)];
    let (others, right) = beside(Key::Term(2), None, &rows);
    assert_eq!(others, [Key::Term(1), Key::Term(3)]);
    assert_eq!(right, [Key::Term(3)]);
}

#[test]
fn a_files_own_place_is_its_path_in_its_project() {
    let project = std::path::Path::new("/work/shop");
    let file = Key::Term(crate::browser::FILE + 1);
    assert_eq!(
        own_file(file, Some(project), "src/cart.rs"),
        Some(project.join("src/cart.rs"))
    );
    // A page and a terminal have no file of their own, and a file with no project none known.
    assert_eq!(
        own_file(Key::Web(1), Some(project), "https://example.com/"),
        None
    );
    assert_eq!(own_file(Key::Term(1), Some(project), ""), None);
    assert_eq!(own_file(file, None, "src/cart.rs"), None);
}
