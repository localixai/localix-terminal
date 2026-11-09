//! Tests of the crate's own helpers.

use super::*;

#[test]
fn a_token_is_32_hex_characters_never_twice_the_same() {
    let tokens: Vec<String> = (0..64).map(|_| token()).collect();
    for token in &tokens {
        assert_eq!(token.len(), 32, "{token}");
        assert!(
            token
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "{token}"
        );
    }
    let mut unique = tokens.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), tokens.len());
}

#[test]
fn random_bytes_cover_every_byte_asked_for() {
    // 4096 bytes all zero, or a byte left as it started in every one of 64 draws, is a buffer
    // the source did not fill.
    assert!(random_bytes::<4096>().iter().any(|byte| *byte != 0));
    let mut touched = [false; 33];
    for _ in 0..64 {
        for (seen, byte) in touched.iter_mut().zip(random_bytes::<33>()) {
            *seen |= byte != 0;
        }
    }
    assert!(touched.iter().all(|seen| *seen), "{touched:?}");
}
