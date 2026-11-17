//! Tests of `carry`: how the side's words are read, and what each does in each state.

use super::{Do, Phase, Said, next, said};
use crate::browser::strip::Key;

fn over() -> Said<'static> {
    Said::Over {
        key: Key::Term(2),
        at: (400.0, 120.0),
        screen: Some((900.0, 300.0)),
    }
}

fn drop() -> Said<'static> {
    Said::Drop {
        key: Key::Term(2),
        at: (400.0, 120.0),
        screen: None,
    }
}

/// The states and what was done, from nothing carried, through `words` (with whether the
/// pointer is past the window's edge).
fn run(words: &[(Said<'_>, bool)]) -> Vec<(Option<Phase>, Do)> {
    let mut now = None;
    let mut done = vec![];
    for (word, past) in words {
        let step = next(now, word, *past);
        now = step.0;
        done.push(step);
    }
    done
}

fn lift() -> Said<'static> {
    said("drag-start t2 10 12 0 80 240 44 600").unwrap()
}

#[test]
fn the_side_s_words_are_read_with_the_screen_s_point_and_a_tree_entry_s_lift_names_no_pane() {
    assert_eq!(
        said("drop-over w1 -30 12 400 220"),
        Some(Said::Over {
            key: Key::Web(1),
            at: (-30.0, 12.0),
            screen: Some((400.0, 220.0))
        })
    );
    assert_eq!(
        said("drop t2 5 6"),
        Some(Said::Drop {
            key: Key::Term(2),
            at: (5.0, 6.0),
            screen: None
        })
    );
    assert_eq!(said("drop-over w1 x 1"), None);
    // A file dropped from the tree is the tree's word, not a carried tab's.
    assert_eq!(said("drop 12 34 /work/a.txt"), None);
    assert_eq!(
        lift(),
        Said::Lift {
            key: Some(Key::Term(2)),
            grab: (10.0, 12.0),
            rect: (0.0, 80.0, 240.0, 44.0),
            inner: Some(600.0)
        }
    );
    assert_eq!(
        said("drag-start 3 4 0 10 200 24"),
        Some(Said::Lift {
            key: None,
            grab: (3.0, 4.0),
            rect: (0.0, 10.0, 200.0, 24.0),
            inner: None
        })
    );
    assert_eq!(said("slot end"), Some(Said::Slot(None)));
    assert_eq!(said("slot t4"), Some(Said::Slot(Some("t4"))));
    assert_eq!(said("drag-at 10 20"), Some(Said::At((10.0, 20.0))));
    assert_eq!(said("move t2 end"), None);
}

#[test]
fn over_the_panes_and_back_to_the_list_puts_the_panes_back_and_writes_nothing() {
    let done = run(&[
        (lift(), false),
        (over(), false),
        (over(), false),
        (Said::Off, false),
        (Said::End, false),
    ]);
    let does: Vec<Do> = done.iter().map(|step| step.1).collect();
    assert_eq!(does, [Do::Lift, Do::Enter, Do::Aim, Do::Leave, Do::End]);
    assert_eq!(done[3].0, Some(Phase::Lifted));
    assert_eq!(done[4].0, None);
}

#[test]
fn let_go_over_the_panes_lands_once_and_the_end_after_it_changes_nothing() {
    let done = run(&[
        (lift(), false),
        (over(), false),
        (drop(), false),
        (over(), false),
        (Said::End, false),
    ]);
    let does: Vec<Do> = done.iter().map(|step| step.1).collect();
    assert_eq!(does, [Do::Lift, Do::Enter, Do::Land, Do::Nothing, Do::End]);
}

#[test]
fn escape_in_each_state_lands_nothing() {
    // Escape is the page's `drop-off` (when out of its list), then its end.
    let escape = [(Said::Off, false), (Said::End, false)];
    let lifted: Vec<Do> = run(&[(lift(), false), escape[0].clone(), escape[1].clone()])
        .iter()
        .map(|step| step.1)
        .collect();
    assert_eq!(lifted, [Do::Lift, Do::Nothing, Do::End]);
    let over_panes: Vec<Do> = run(&[
        (lift(), false),
        (over(), false),
        escape[0].clone(),
        (Said::End, false),
    ])
    .iter()
    .map(|step| step.1)
    .collect();
    assert_eq!(over_panes, [Do::Lift, Do::Enter, Do::Leave, Do::End]);
    let out: Vec<Do> = run(&[
        (lift(), false),
        (over(), true),
        escape[0].clone(),
        (Said::End, false),
    ])
    .iter()
    .map(|step| step.1)
    .collect();
    assert_eq!(out, [Do::Lift, Do::ComeOut, Do::TakeBack, Do::End]);
    // An end with no word before it puts the panes back too.
    let ended = run(&[(lift(), false), (over(), false), (Said::End, false)]);
    assert_eq!(ended[2], (None, Do::Leave));
}

#[test]
fn past_the_edge_from_the_list_or_the_panes_it_comes_out_and_follows_till_let_go() {
    let done = run(&[
        (lift(), false),
        (over(), false),
        (over(), true),
        (over(), false),
        (Said::Slot(Some("t4")), false),
        (drop(), false),
        (Said::End, false),
    ]);
    let does: Vec<Do> = done.iter().map(|step| step.1).collect();
    assert_eq!(
        does,
        [
            Do::Lift,
            Do::Enter,
            Do::ComeOut,
            Do::Move,
            Do::Slot,
            Do::LetGo,
            Do::End
        ]
    );
    // Out and back over its list: given back, carried in its list again, nothing written.
    let back = run(&[
        (lift(), false),
        (over(), true),
        (Said::Off, false),
        (over(), false),
    ]);
    let does: Vec<Do> = back.iter().map(|step| step.1).collect();
    assert_eq!(does, [Do::Lift, Do::ComeOut, Do::TakeBack, Do::Enter]);
}

#[test]
fn nothing_carried_does_nothing_and_a_new_lift_starts_again() {
    assert_eq!(next(None, &over(), false), (None, Do::Nothing));
    assert_eq!(next(None, &Said::End, false), (None, Do::Nothing));
    assert_eq!(
        next(Some(Phase::Over), &lift(), false),
        (Some(Phase::Lifted), Do::Lift)
    );
}
