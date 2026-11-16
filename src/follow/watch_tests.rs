//! Tests of `watch`: which lines of a list are tabs.

use super::tabs;

#[test]
fn the_tabs_are_the_lines_a_tab_s_key_begins() {
    let listed = "Tabs of shop:\nw1 Cart · https://shop.test\nt2 zsh · ~/shop\nb1 Board\n\
                  tabs: 3\nwindow open\nt grep";
    assert_eq!(
        tabs(listed),
        ["w1 Cart · https://shop.test", "t2 zsh · ~/shop", "b1 Board"]
    );
    assert!(tabs("").is_empty());
}
