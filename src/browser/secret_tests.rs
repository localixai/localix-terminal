//! Tests of `secret`.

use std::collections::BTreeSet;

use super::*;

#[test]
fn a_word_is_32_hex_characters_and_never_the_same_twice() {
    let words: BTreeSet<String> = (0..64).map(|_| word()).collect();
    assert_eq!(words.len(), 64);
    for word in &words {
        assert_eq!(word.len(), 32);
        assert!(word.chars().all(|c| c.is_ascii_hexdigit()), "{word}");
    }
}

#[test]
fn call_ids_do_not_follow_one_another() {
    // A page that could count on the next call's number could answer it first.
    let ids: Vec<u64> = (0..64).map(|_| call_id()).collect();
    assert!(ids.iter().all(|id| *id != 0));
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
    let stepped = ids
        .windows(2)
        .filter(|pair| pair[1].wrapping_sub(pair[0]) < 1 << 16);
    assert!(stepped.count() < 2, "{ids:?}");
    // Past what a page's numbers hold exactly: an id goes to the page as text.
    assert!(ids.iter().any(|id| *id > 1 << 53));
}
