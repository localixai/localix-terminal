//! A frame laid out: the gutter and the grid it leaves, grounds only where a cell has its own,
//! runs split by face and colour with each character at its column, wide characters, bold in
//! the bright colours, inverse, faint and invisible text, lines, the selection and the matches,
//! the cursor, and the scrollbar.

use super::{Line, Paint, grid, gutter, row};
use crate::browser::term_view::theme::Theme;
use crate::surface::{Cell, Color, Flags, Frame, Rgb, Row, Span, Style, Underline, Wide};

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
fn the_gutter_is_the_pages_and_a_full_screen_program_has_none_but_below() {
    assert_eq!((gutter(false).top, gutter(false).left), (8, 16));
    assert_eq!(
        (gutter(true).top, gutter(true).left, gutter(true).bottom),
        (0, 0, 8)
    );
    assert_eq!(grid(832.0, 656.0, (8.0, 16.0), false), (100, 40));
    assert_eq!(grid(800.0, 648.0, (8.0, 16.0), true), (100, 40));
    assert_eq!(
        grid(1.0, 1.0, (8.0, 16.0), false),
        (2, 1),
        "never less than two by one"
    );
}

#[test]
fn a_plain_row_is_one_run_on_the_clear_ground() {
    let frame = frame_of(vec![row_of(0, &plain("ls -la"))]);
    let (paint, _) = row(&frame.rows[0], &frame, Theme::new(true));
    assert!(
        paint.fills.is_empty(),
        "the terminal's own ground is never painted"
    );
    let texts: Vec<(&str, &[(u32, u16)])> = paint
        .runs
        .iter()
        .map(|run| (run.text.as_str(), run.columns.as_slice()))
        .collect();
    assert_eq!(
        texts,
        [
            ("ls", &[(0, 0), (1, 1)][..]),
            ("-la", &[(0, 3), (1, 4), (2, 5)][..])
        ]
    );
}

#[test]
fn runs_split_where_the_colour_changes_and_wide_characters_take_two_columns() {
    let red = Style {
        fg: Color::Palette(1),
        ..Style::default()
    };
    let frame = frame_of(vec![row_of(
        0,
        &[("a", red), ("日", red), ("b", Style::default())],
    )]);
    let (paint, _) = row(&frame.rows[0], &frame, Theme::new(true));
    assert_eq!(paint.runs.len(), 2);
    assert_eq!(paint.runs[0].text, "a日");
    assert_eq!(paint.runs[0].columns, [(0, 0), (1, 1)]);
    assert_eq!(
        paint.runs[1].columns,
        [(0, 3)],
        "after the wide character's two columns"
    );
    assert_eq!(paint.runs[0].paint.rgb, Rgb::hex(0xff7b72));
}

#[test]
fn bold_in_one_of_the_first_eight_colours_is_drawn_bright() {
    let style = Style {
        fg: Color::Palette(1),
        flags: Flags(Flags::BOLD),
        ..Style::default()
    };
    let frame = frame_of(vec![row_of(0, &[("x", style)])]);
    let (paint, _) = row(&frame.rows[0], &frame, Theme::new(true));
    assert_eq!(paint.runs[0].paint.rgb, Rgb::hex(0xffa198));
    assert!(paint.runs[0].bold);
}

#[test]
fn inverse_faint_and_invisible_text() {
    let inverse = Style {
        flags: Flags(Flags::INVERSE),
        ..Style::default()
    };
    let faint = Style {
        flags: Flags(Flags::FAINT),
        ..Style::default()
    };
    let hidden = Style {
        flags: Flags(Flags::INVISIBLE),
        ..Style::default()
    };
    let frame = frame_of(vec![row_of(
        0,
        &[("i", inverse), ("f", faint), ("h", hidden)],
    )]);
    let theme = Theme::new(true);
    let (paint, _) = row(&frame.rows[0], &frame, theme);
    assert_eq!(
        paint.fills[0].paint,
        Paint::solid(theme.fg()),
        "the ground takes the text colour"
    );
    assert_eq!(
        paint.runs[0].paint.rgb,
        theme.bg(),
        "the text takes the ground's"
    );
    assert_eq!(paint.runs[1].paint.alpha, 128);
    assert_eq!(paint.runs.len(), 2, "invisible text is not drawn");
}

#[test]
fn lines_span_the_cells_that_have_them() {
    let under = Style {
        underline: Underline::Curly,
        ..Style::default()
    };
    let strike = Style {
        flags: Flags(Flags::STRIKE),
        ..Style::default()
    };
    let frame = frame_of(vec![row_of(
        0,
        &[("a", under), ("b", under), ("c", strike)],
    )]);
    let (paint, _) = row(&frame.rows[0], &frame, Theme::new(true));
    let lines: Vec<(u16, u16, Line)> = paint.decos.iter().map(|d| (d.from, d.to, d.line)).collect();
    assert_eq!(
        lines,
        [(0, 2, Line::Under(Underline::Curly)), (2, 3, Line::Strike)]
    );
}

#[test]
fn the_selection_and_the_matches_lie_over_the_cells() {
    let mut frame = frame_of(vec![row_of(0, &plain("find me here"))]);
    frame.rows[0].selected = Some((0, 3));
    frame.matches = vec![
        Span {
            y: 0,
            from: 5,
            to: 6,
        },
        Span {
            y: 0,
            from: 8,
            to: 11,
        },
    ];
    frame.current = Some(frame.matches[1]);
    let theme = Theme::new(true);
    let (paint, key) = row(&frame.rows[0], &frame, theme);
    let fills: Vec<(u16, u16)> = paint.fills.iter().map(|f| (f.from, f.to)).collect();
    assert_eq!(fills, [(0, 4), (5, 7), (8, 12)]);
    assert_eq!(paint.fills[0].paint, theme.selection());
    assert_eq!(paint.fills[2].paint, theme.found(true));
    assert_eq!(paint.decos[0].line, Line::Edge);
    frame.current = None;
    assert_ne!(
        row(&frame.rows[0], &frame, theme).1,
        key,
        "what is drawn differs, so the key"
    );
}

#[test]
fn an_emoji_sequence_over_several_cells_is_one_run_to_shape_as_one() {
    // A family: three people joined, each a wide cell with its joiner, as the core keeps them.
    let mut cells = vec![];
    for person in ["👨\u{200d}", "👩\u{200d}", "👧"] {
        cells.push((person, Style::default()));
    }
    let mut row = Row {
        dirty: true,
        ..Row::default()
    };
    for (text, style) in cells {
        let from = u32::try_from(row.text.len()).unwrap();
        row.text.push_str(text);
        let len = u16::try_from(text.len()).unwrap();
        row.cells.push(Cell {
            from,
            len,
            wide: Wide::Wide,
            style,
        });
        row.cells.push(Cell {
            wide: Wide::Tail,
            ..Cell::default()
        });
    }
    let frame = frame_of(vec![row]);
    let (paint, _) = row_paint(&frame);
    assert_eq!(paint.runs.len(), 1, "{:?}", paint.runs);
    assert_eq!(paint.runs[0].text, "👨\u{200d}👩\u{200d}👧");
    assert_eq!(paint.runs[0].columns, [(0, 0), (7, 2), (14, 4)]);
    assert_eq!(paint.runs[0].end, 6, "it covers its six columns");
}

#[test]
fn the_link_pointed_at_is_underlined_in_its_text_colour() {
    let mut frame = frame_of(vec![row_of(0, &plain("see a.test"))]);
    let (plain_paint, plain_key) = row_paint(&frame);
    frame.link = vec![Span {
        y: 0,
        from: 4,
        to: 9,
    }];
    let (paint, key) = row_paint(&frame);
    let under: Vec<(u16, u16)> = (paint.decos.iter())
        .filter(|deco| deco.line == Line::Under(Underline::Single))
        .map(|deco| (deco.from, deco.to))
        .collect();
    assert_eq!(under, [(4, 10)]);
    assert!(plain_paint.decos.is_empty());
    assert_ne!(key, plain_key, "the row is drawn again");
}

fn row_paint(frame: &Frame) -> (super::RowPaint, u64) {
    row(&frame.rows[0], frame, Theme::new(true))
}
