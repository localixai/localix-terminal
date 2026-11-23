//! The places a raw cell's fields are read from: the linked library's manifest says them, and
//! reading a cell by them gives what the library says of it for every kind of cell.

use ghostty_vt_sys as vt;

use super::{asked, places};

#[test]
fn the_manifest_of_the_library_linked_says_where_a_cells_fields_lie() {
    assert!(
        places().is_some(),
        "a frame would ask the library for every field"
    );
}

/// Cells as a screen holds them: empty, a letter, a wide character's two halves, a codepoint
/// of a cluster, grounds alone, with and without a style, and every bit pattern near the edges
/// of each field.
#[test]
fn a_cell_read_by_its_places_is_what_the_library_says_of_it() {
    let places = places().unwrap();
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for round in 0..200_000u64 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        // Every eighth word is random in full; the rest keep the bits the library writes
        // (below 48) so the kinds of cells a screen holds come up often.
        let raw: vt::Cell = if round % 8 == 0 {
            seed
        } else {
            seed & ((1 << 48) - 1)
        };
        assert_eq!(places.read(raw), asked(raw), "cell {raw:#018x}");
    }
}
