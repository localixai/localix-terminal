//! The rows a view keeps: a row drawn once is not drawn again while its look is the same, even
//! marked dirty (as every row is after the view scrolls), blank rows share one picture, and what
//! leaves the view is let go of.

use super::Cache;
use crate::browser::term_view::theme::Theme;
use crate::surface::{Cell, Frame, Row, RowId};

fn row(id: u128, y: u16, text: &str) -> Row {
    let mut row = Row {
        id: RowId(id),
        y,
        dirty: true,
        ..Row::default()
    };
    for (at, _) in text.char_indices() {
        let from = u32::try_from(at).unwrap();
        row.cells.push(Cell {
            from,
            len: 1,
            ..Cell::default()
        });
    }
    row.text = text.to_string();
    row
}

fn frame(rows: Vec<Row>) -> Frame {
    Frame {
        rows,
        ..Frame::default()
    }
}

#[test]
fn rows_scrolled_and_marked_dirty_are_not_drawn_again() {
    let theme = Theme::new(true);
    let mut cache: Cache<String> = Cache::default();
    let mut drawn = vec![];
    let mut draw = |row: &Row, _: &_| {
        drawn.push(row.text.clone());
        Some(row.text.clone())
    };
    let first = cache.keep(
        &frame(vec![row(1, 0, "a"), row(2, 1, "b"), row(3, 2, "")]),
        theme,
        &mut draw,
    );
    assert_eq!((first.drawn, first.kept), (3, 0));
    // Scrolled by one: every row dirty, two of them the same as before.
    let second = cache.keep(
        &frame(vec![row(2, 0, "b"), row(3, 1, ""), row(4, 2, "c")]),
        theme,
        &mut draw,
    );
    assert_eq!((second.drawn, second.kept), (1, 2));
    assert_eq!(drawn, ["a", "b", "", "c"]);
    assert!(
        cache.picture(RowId(1)).is_none(),
        "the row that left is let go of"
    );
    assert_eq!(cache.pictures().count(), 3);
}

#[test]
fn a_clean_row_keeps_its_picture_and_blank_rows_share_one() {
    let theme = Theme::new(true);
    let mut cache: Cache<u32> = Cache::default();
    let mut count = 0;
    let mut draw = |_: &Row, _: &_| {
        count += 1;
        Some(count)
    };
    cache.keep(
        &frame(vec![row(1, 0, ""), row(2, 1, ""), row(3, 2, "x")]),
        theme,
        &mut draw,
    );
    assert_eq!(cache.pictures().count(), 2, "two blank rows, one picture");
    let mut clean = row(3, 2, "");
    clean.dirty = false;
    let again = cache.keep(
        &frame(vec![row(1, 0, ""), row(2, 1, ""), clean]),
        theme,
        &mut draw,
    );
    assert_eq!((again.drawn, again.kept), (0, 3));
    assert_eq!(cache.picture(RowId(3)).map(|(_, p)| p), Some(&2));
}
