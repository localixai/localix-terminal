//! Tests of where a command goes: a pane's to its window, a press to the window pressed, a change
//! of all to every window, anything else to the window in use.

use std::sync::mpsc::sync_channel;

use super::super::host::Cmd;
use super::super::strip::{Click, Key};
use super::{Route, route};

#[test]
fn a_command_goes_to_its_panes_window_a_press_to_its_window_a_change_to_all() {
    let pane = |key: &str| Route::Pane(key.to_string());
    assert_eq!(route(&Cmd::Paint(3)), pane("t3"));
    assert_eq!(route(&Cmd::Titled(2, "zsh".into())), pane("t2"));
    assert_eq!(
        route(&Cmd::Navigate("https://a".into(), Some(4))),
        pane("w4")
    );
    assert_eq!(route(&Cmd::Reveal(1)), pane("w1"));
    let (reply, _) = sync_channel(1);
    assert_eq!(
        route(&Cmd::Act(Click::Show(Key::Web(5)), reply)),
        pane("w5")
    );
    assert_eq!(route(&Cmd::Pressed(77, 1.0, 2.0)), Route::Pressed(77));
    assert_eq!(route(&Cmd::Strip), Route::All);
    assert_eq!(route(&Cmd::Arranged(vec![], vec![])), Route::All);
    // No pane named: the window in use.
    assert_eq!(
        route(&Cmd::Navigate("https://a".into(), None)),
        Route::Current
    );
    assert_eq!(route(&Cmd::Clicked("show w1".into())), Route::Current);
    assert_eq!(route(&Cmd::NewWindow(2)), Route::Current);
}
