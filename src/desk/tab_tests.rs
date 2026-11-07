//! Tests of tab keys at the tool boundary.

use super::*;

#[test]
fn a_tab_key_is_normalized_but_its_number_is_decimal() {
    let tab = parse(" W12 ").unwrap();
    assert_eq!(
        (tab.key.as_str(), tab.kind.name(), tab.id),
        ("w12", "browser", 12)
    );
    for key in ["w+1", "t-1", "b", "x1", "w1 extra", "w4294967296"] {
        assert!(parse(key).is_none(), "{key}");
    }
}
