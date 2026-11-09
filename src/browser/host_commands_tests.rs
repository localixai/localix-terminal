//! Tests of each command's one declaration: the window it goes to, and whether the window is
//! kept on its project and drawn at once after it (it settles), drawn once the turn is over
//! (it tells), or neither.

use std::sync::Arc;
use std::sync::mpsc::sync_channel;

use super::super::super::presence::Event;
use super::super::super::strip::{Click, Key};
use super::super::super::windows::Route;
use super::super::{Confirm, Noted, Window};
use super::{Cmd, Drawn};

/// Every command but those that name a pane or a window, each once, with how it is drawn.
fn unnamed() -> Vec<(Cmd, Route, Drawn)> {
    let (opened, _) = sync_channel(1);
    let (loaded, _) = sync_channel(1);
    let (shot, _) = sync_channel(1);
    let window = Window {
        url: "about:blank".into(),
        title: String::new(),
        headless: true,
        viewport: super::super::Viewport::DESKTOP,
    };
    let eval = Cmd::Eval {
        id: 1,
        script: String::new(),
        failed: "f",
        panel: None,
        tab: None,
    };
    vec![
        (
            Cmd::Open {
                window,
                reply: opened,
            },
            Route::Current,
            Drawn::Itself,
        ),
        (eval, Route::Current, Drawn::Itself),
        (
            Cmd::Navigate {
                url: String::new(),
                tab: None,
                reply: loaded,
            },
            Route::Current,
            Drawn::Itself,
        ),
        (
            Cmd::NewTab {
                url: String::new(),
                reply: None,
                opener: None,
            },
            Route::Current,
            Drawn::Settles,
        ),
        (Cmd::Close, Route::Current, Drawn::Settles),
        (Cmd::Frame(None, None), Route::Current, Drawn::Itself),
        (
            Cmd::Capture {
                clip: None,
                reply: shot,
                tab: None,
            },
            Route::Current,
            Drawn::Itself,
        ),
        (
            Cmd::Panel((1, String::new(), String::new(), Arc::default())),
            Route::Current,
            Drawn::Settles,
        ),
    ]
}

/// The rest of them.
fn unnamed_too() -> Vec<(Cmd, Route, Drawn)> {
    let (dressed, _) = sync_channel(1);
    let (listed, _) = sync_channel(1);
    let (acted, _) = sync_channel(1);
    let (presented, _) = sync_channel(1);
    let (asked, _) = sync_channel(1);
    let noted = Noted {
        tab: 0,
        said: String::new(),
        clip: None,
    };
    let confirm = Confirm {
        title: String::new(),
        detail: String::new(),
        yes: String::new(),
    };
    vec![
        (Cmd::Clicked(String::new()), Route::Current, Drawn::Settles),
        (
            Cmd::SideClicked(String::new()),
            Route::Current,
            Drawn::Settles,
        ),
        (
            Cmd::NavClicked(String::new()),
            Route::Current,
            Drawn::Settles,
        ),
        (Cmd::Restyle, Route::All, Drawn::Itself),
        (Cmd::Noted(noted), Route::Current, Drawn::Itself),
        (Cmd::Strip, Route::All, Drawn::Told),
        (Cmd::Offered("x", true), Route::All, Drawn::Itself),
        (Cmd::Go(String::new()), Route::Current, Drawn::Settles),
        (Cmd::Arranged(vec![], vec![]), Route::All, Drawn::Settles),
        (Cmd::NewWindow(2), Route::Current, Drawn::Itself),
        (Cmd::Adopt, Route::Current, Drawn::Settles),
        (Cmd::Dressed(dressed), Route::Current, Drawn::Itself),
        (Cmd::Listing(listed), Route::Current, Drawn::Itself),
        (Cmd::Act(Click::Back, acted), Route::Current, Drawn::Settles),
        (Cmd::Presence(Event::Acting), Route::Current, Drawn::Itself),
        (Cmd::Opened, Route::Current, Drawn::Itself),
        (Cmd::Presented(presented), Route::Current, Drawn::Itself),
        (Cmd::Confirm(confirm, asked), Route::Current, Drawn::Itself),
        (Cmd::Exit(0), Route::Current, Drawn::Itself),
    ]
}

/// The commands that name a pane or a window, with the one they go to.
fn named() -> Vec<(Cmd, Route, Drawn)> {
    let pane = |key: &str| Route::Pane(key.to_string());
    let (loaded, _) = sync_channel(1);
    let (switched, _) = sync_channel(1);
    let (closed, _) = sync_channel(1);
    let (shot, _) = sync_channel(1);
    let (acted, _) = sync_channel(1);
    let (shut, _) = sync_channel(1);
    let eval = |panel, tab| Cmd::Eval {
        id: 1,
        script: String::new(),
        failed: "f",
        panel,
        tab,
    };
    vec![
        (eval(Some(3), None), pane("t3"), Drawn::Itself),
        (eval(None, Some(4)), pane("w4"), Drawn::Itself),
        (Cmd::Paint(3), pane("t3"), Drawn::Itself),
        (Cmd::Unpanel(3), pane("t3"), Drawn::Settles),
        (Cmd::PanelMenu(3, (1, 2), vec![]), pane("t3"), Drawn::Itself),
        (Cmd::Titled(2, "zsh".into()), pane("t2"), Drawn::Told),
        (
            Cmd::Navigate {
                url: String::new(),
                tab: Some(4),
                reply: loaded,
            },
            pane("w4"),
            Drawn::Itself,
        ),
        (Cmd::Reveal(1), pane("w1"), Drawn::Itself),
        (
            Cmd::SwitchTab {
                id: 5,
                reply: switched,
            },
            pane("w5"),
            Drawn::Settles,
        ),
        (
            Cmd::CloseTab {
                id: 6,
                reply: closed,
            },
            pane("w6"),
            Drawn::Settles,
        ),
        (Cmd::Frame(Some(7), None), pane("w7"), Drawn::Itself),
        (
            Cmd::Capture {
                clip: None,
                reply: shot,
                tab: Some(8),
            },
            pane("w8"),
            Drawn::Itself,
        ),
        (
            Cmd::Act(Click::Show(Key::Web(5)), acted),
            pane("w5"),
            Drawn::Settles,
        ),
        (
            Cmd::Act(Click::Close(Key::Term(9)), shut),
            pane("t9"),
            Drawn::Settles,
        ),
        (
            Cmd::Pressed(77, 1.0, 2.0),
            Route::Pressed(77),
            Drawn::Itself,
        ),
    ]
}

#[test]
fn each_command_declares_the_window_it_goes_to_and_how_it_is_drawn() {
    for (at, (command, route, drawn)) in unnamed()
        .into_iter()
        .chain(unnamed_too())
        .chain(named())
        .enumerate()
    {
        assert_eq!(
            (command.route(), command.drawn()),
            (route, drawn),
            "command {at}"
        );
    }
}
