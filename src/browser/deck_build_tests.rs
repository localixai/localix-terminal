//! Tests of what a part of the window posts: whose points they are.

use super::{Part, posted};
use crate::browser::host::Cmd;

#[test]
fn each_part_that_lies_elsewhere_says_whose_points_its_are() {
    // The documents' row lies over a pane, the side under the bar, the toolbar over the pages:
    // a menu asked for at a point of one of them is placed from where that part lies.
    let said = || "menu w1 30 12".to_string();
    assert!(matches!(posted(Part::Docs, said()), Cmd::DocsClicked(_)));
    assert!(matches!(posted(Part::Side, said()), Cmd::SideClicked(_)));
    assert!(matches!(posted(Part::Nav, said()), Cmd::NavClicked(_)));
    assert!(matches!(posted(Part::Strip, said()), Cmd::Clicked(_)));
}
