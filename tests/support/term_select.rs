//! Selecting in a terminal with the pointer, in a real window: a drag selects what it covers, a
//! double click a word, a triple click a line, a drag with Option and Command a block; Copy puts
//! the selection on the pasteboard (the person's put back after), Select All takes the screen;
//! and the history is reached by dragging the scrollbar's slider and by the wheel.

use std::time::Duration;

use llxterm::browser::{Browser, Edit, Pointing, TermAnswer, TermProbe};

use super::pasteboard::{Saved, pasted};
use super::terminal_view::{COMMAND, opened, seen_in};

/// The terminals' ids: apart from the other scenes' and the desk's.
const ID: u32 = 907;
const LONG: u32 = 908;
const OPTION: usize = (1 << 19) | 0x20;

/// The pointer at a cell of view `id`, as `pointing` says, with `flags` held.
fn point(browser: &Browser, pointing: Pointing, (column, row): (u16, u16), flags: usize) {
    let probe = TermProbe::Pointer {
        pointing,
        column,
        row,
        flags,
    };
    assert_eq!(browser.term_probe(ID, probe), Some(TermAnswer::Done));
}

/// `times` clicks in a row at a cell: one, a double or a triple click.
fn click(browser: &Browser, at: (u16, u16), times: usize) {
    for _ in 0..times {
        point(browser, Pointing::Down, at, 0);
        point(browser, Pointing::Up, at, 0);
    }
    // The next press is a click of its own.
    std::thread::sleep(Duration::from_millis(600));
}

fn selected(browser: &Browser, want: &str) {
    seen_in(browser, ID, |seen| seen.selected.as_deref() == Some(want));
}

fn edit(browser: &Browser, id: u32, edit: Edit) {
    assert_eq!(
        browser.term_probe(id, TermProbe::Edit(edit)),
        Some(TermAnswer::Done)
    );
}

pub(crate) fn scenario(browser: &Browser) {
    let session = opened(
        browser,
        ID,
        "printf 'alpha beta gamma\\nsecond line here\\nthird row\\n'; exec sleep 60",
    );
    seen_in(browser, ID, |seen| {
        seen.rows.iter().any(|row| row.trim() == "third row")
    });

    // A drag over the first word selects it (a cell is taken once the pointer passes its middle).
    point(browser, Pointing::Down, (0, 0), 0);
    point(browser, Pointing::Dragged, (5, 0), 0);
    point(browser, Pointing::Up, (5, 0), 0);
    selected(browser, "alpha");
    std::thread::sleep(Duration::from_millis(600));
    // A double click a word, a triple click the line.
    click(browser, (7, 0), 2);
    selected(browser, "beta");
    click(browser, (3, 1), 3);
    selected(browser, "second line here");
    // Option and Command held, a drag selects a block of the same columns of each row.
    point(browser, Pointing::Down, (0, 0), OPTION | COMMAND);
    point(browser, Pointing::Dragged, (5, 2), OPTION | COMMAND);
    point(browser, Pointing::Up, (5, 2), OPTION | COMMAND);
    selected(browser, "alpha\nsecon\nthird");

    // Copy puts it on the pasteboard; what the person had there comes back after.
    {
        let _kept = Saved::now("alpha\nsecon\nthird");
        edit(browser, ID, Edit::Copy);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while pasted() != "alpha\nsecon\nthird" {
            assert!(
                std::time::Instant::now() < deadline,
                "Copy left {:?}",
                pasted()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    // Select All takes all of it.
    edit(browser, ID, Edit::SelectAll);
    seen_in(browser, ID, |seen| {
        seen.selected
            .as_ref()
            .is_some_and(|all| all.starts_with("alpha beta gamma\nsecond line here\nthird row"))
    });
    session.close();
    history(browser);
}

/// The scrollbar's slider dragged to the top shows the oldest rows; the wheel goes back down.
fn history(browser: &Browser) {
    let session = opened(browser, LONG, "seq 1 400; exec sleep 60");
    let bottom = seen_in(browser, LONG, |seen| {
        seen.history > 300 && seen.rows.iter().any(|row| row.trim() == "400")
    });
    assert!(bottom.offset > 0, "{bottom:#?}");
    let bar = |pointing: Pointing, percent: u8| {
        let probe = TermProbe::Bar { pointing, percent };
        assert_eq!(browser.term_probe(LONG, probe), Some(TermAnswer::Done));
    };
    bar(Pointing::Down, 98);
    bar(Pointing::Dragged, 50);
    bar(Pointing::Dragged, 0);
    bar(Pointing::Up, 0);
    let top = seen_in(browser, LONG, |seen| seen.offset == 0);
    assert_eq!(
        top.rows.first().map(|row| row.trim()),
        Some("1"),
        "{top:#?}"
    );
    // The wheel turned down, toward the newest rows.
    let wheel = TermProbe::Wheel(-3);
    assert_eq!(browser.term_probe(LONG, wheel), Some(TermAnswer::Done));
    let down = seen_in(browser, LONG, |seen| seen.offset > 0);
    assert!(down.offset < bottom.offset, "{down:#?}");
    session.close();
}
