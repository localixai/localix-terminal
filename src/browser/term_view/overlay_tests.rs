//! What lies over the rows: the cursor as a block with its character in the ground's colour, an
//! outline in a pane not in use, none when hidden; the scrollbar only with history, its slider
//! never shorter than 20 points; once the program ended, no cursor and a dim line that says how,
//! where the cursor was; the cursor blinks only as asked, in use, and without less motion.

use super::{blinks, cursor, ended, scrollbar};
use crate::browser::term_view::theme::Theme;
use crate::surface::{Cell, Frame, Row, Scrollbar, Shape, Style, Wide};

/// A row of `cells` (each its text and style).
fn row_of(y: u16, cells: &[(&str, Style)]) -> Row {
    let mut row = Row {
        y,
        dirty: true,
        ..Row::default()
    };
    for (text, style) in cells {
        let wide = match text.chars().next().map(|c| c as u32) {
            Some(0x3000..=0x9fff) => Wide::Wide,
            _ => Wide::Narrow,
        };
        let from = u32::try_from(row.text.len()).unwrap();
        row.text.push_str(text);
        let len = u16::try_from(text.len()).unwrap();
        row.cells.push(Cell {
            from,
            len,
            wide,
            style: *style,
        });
        if wide == Wide::Wide {
            row.cells.push(Cell {
                wide: Wide::Tail,
                ..Cell::default()
            });
        }
    }
    row
}

fn plain(text: &str) -> Vec<(&str, Style)> {
    text.split("")
        .filter(|c| !c.is_empty())
        .map(|c| (c, Style::default()))
        .collect()
}

fn frame_of(rows: Vec<Row>) -> Frame {
    let mut frame = Frame {
        rows,
        ..Frame::default()
    };
    let theme = Theme::new(true).palette();
    frame.colors.foreground = theme.foreground;
    frame.colors.background = theme.background;
    for (at, colour) in theme.ansi.iter().enumerate() {
        frame.colors.palette[at] = *colour;
    }
    frame
}

#[test]
fn the_cursor_is_a_block_with_its_character_in_the_grounds_colour_and_an_outline_when_idle() {
    let mut frame = frame_of(vec![row_of(0, &plain("ab"))]);
    frame.cursor.at = Some((1, 0));
    let mut theme = Theme::new(true);
    let drawn = cursor(&frame, theme).unwrap();
    assert_eq!(
        (drawn.column, drawn.y, drawn.width, drawn.shape),
        (1, 0, 1, Shape::Block)
    );
    assert_eq!(drawn.paint.rgb, theme.fg());
    let under = drawn.under.unwrap();
    assert_eq!((under.text.as_str(), under.paint.rgb), ("b", theme.bg()));
    theme.active = false;
    assert_eq!(cursor(&frame, theme).unwrap().shape, Shape::Hollow);
    frame.cursor.at = None;
    assert!(cursor(&frame, theme).is_none());
}

#[test]
fn the_scrollbar_shows_only_with_history() {
    let mut frame = frame_of(vec![]);
    frame.scroll = Scrollbar {
        total: 40,
        offset: 0,
        len: 40,
    };
    assert_eq!(scrollbar(&frame, 400.0), None);
    frame.scroll = Scrollbar {
        total: 400,
        offset: 360,
        len: 40,
    };
    assert_eq!(
        scrollbar(&frame, 400.0),
        Some((360.0, 40.0)),
        "at the bottom"
    );
    frame.scroll = Scrollbar {
        total: 100_000,
        offset: 0,
        len: 40,
    };
    assert_eq!(
        scrollbar(&frame, 400.0),
        Some((0.0, 20.0)),
        "never shorter than 20"
    );
}

#[test]
fn a_program_that_ended_has_a_dim_line_where_its_cursor_was_and_no_cursor() {
    let theme = Theme::new(true);
    let mut frame = Frame {
        columns: 40,
        lines: 3,
        rows: vec![row_of(0, &plain("$ make"))],
        ..Frame::default()
    };
    frame.cursor.at = Some((0, 1));
    assert!(ended(&frame, theme).is_none(), "nothing while it runs");
    frame.ended = Some("(the program exited with 2)".into());
    assert!(
        cursor(&frame, theme).is_none(),
        "the line takes the cursor's place"
    );
    let (y, paint) = ended(&frame, theme).unwrap();
    assert_eq!(y, 1, "at the start of the cursor's row");
    let run = &paint.runs[0];
    assert_eq!(run.text, "(the program exited with 2)");
    assert_eq!(run.columns[1], (1, 1));
    assert!(run.paint.alpha < 255, "dim, as faint text is");
    frame.cursor.at = Some((6, 1));
    assert_eq!(
        ended(&frame, theme).unwrap().0,
        2,
        "under a line the cursor is in"
    );
    frame.cursor.at = Some((6, 2));
    assert!(ended(&frame, theme).is_none(), "no row under the last");
}

#[test]
fn the_cursor_blinks_only_as_its_program_asked_in_use_and_with_motion() {
    let mut frame = Frame::default();
    frame.cursor.at = Some((0, 0));
    assert!(!blinks(&frame, true, false), "steady unless asked");
    frame.cursor.blinking = true;
    assert!(blinks(&frame, true, false));
    assert!(
        !blinks(&frame, false, false),
        "not out of use or out of sight"
    );
    assert!(!blinks(&frame, true, true), "not with less motion");
    frame.ended = Some("(the program ended)".into());
    assert!(!blinks(&frame, true, false), "not once the program ended");
}
