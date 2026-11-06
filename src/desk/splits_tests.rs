use std::collections::HashMap;

use super::*;
use crate::store::Store;

#[test]
fn a_split_is_kept_until_the_window_is_whole() {
    let store = Store::memory().unwrap();
    let split = Kept {
        shown: 4,
        beside: 7,
        right: true,
    };
    store.write(|tx| keep(tx, "p1", Some(split))).unwrap();
    assert_eq!(store.read(|db| kept(db, "p1")).unwrap(), Some(split));
    assert_eq!(store.read(|db| kept(db, "p2")).unwrap(), None);
    // Split again, the last is kept.
    let again = Kept {
        shown: 7,
        beside: 4,
        right: false,
    };
    store.write(|tx| keep(tx, "p1", Some(again))).unwrap();
    assert_eq!(store.read(|db| kept(db, "p1")).unwrap(), Some(again));
    store.write(|tx| keep(tx, "p1", None)).unwrap();
    assert_eq!(store.read(|db| kept(db, "p1")).unwrap(), None);
}

#[test]
fn a_split_comes_back_only_with_both_its_tabs() {
    let split = Kept {
        shown: 4,
        beside: 7,
        right: true,
    };
    let mut keys = HashMap::from([(4, "w3".to_string())]);
    assert_eq!(halves(&split, &keys), None);
    keys.insert(7, "t9".to_string());
    assert_eq!(
        halves(&split, &keys),
        Some(Halves {
            shown: "w3",
            beside: "t9",
            right: true
        })
    );
}
