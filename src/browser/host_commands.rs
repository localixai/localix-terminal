//! Commands sent to the native event loop; no window or shared state lives here.

use std::sync::mpsc::SyncSender;

use crate::WindowTab;

use super::super::strip::{Click, Key};
use super::super::windows::Route;
use super::{Arranged, Clip, Confirm, Dressing, Noted, Shot, Window, panel};

pub(in super::super) enum Cmd {
    /// Open the window (once); the reply says whether it opened.
    Open {
        window: Window,
        reply: SyncSender<Result<(), String>>,
    },
    /// A script built by the driver that hands a probe call or its promise to the probe; `false`
    /// or a parse error answers the call at once with `failed` (`probe_not_ready`,
    /// `script_not_started`).
    Eval {
        id: u64,
        script: String,
        failed: &'static str,
        /// In this panel's page instead, answered with the script's value (for checks).
        panel: Option<u32>,
        /// In this browser tab instead of the one acted on.
        tab: Option<u32>,
    },
    /// Load an address: in this tab, else the one acted on. Answered once the load is asked
    /// for, or at once with why it cannot be (the window or the tab is closed).
    Navigate {
        url: String,
        tab: Option<u32>,
        reply: SyncSender<Result<(), String>>,
    },
    /// Open a tab on `url` and show it. A page's own new window (`target=_blank`, `window.open`)
    /// comes here with no reply.
    NewTab {
        url: String,
        reply: Option<SyncSender<Result<u32, String>>>,
        /// The tab whose page asked for it, whose project it is of.
        opener: Option<u32>,
    },
    /// A new tab's page has drawn: the tab draws its own ground again (until then the window's
    /// showed through it, so a blank view never flashes white over a dark window).
    Reveal(u32),
    SwitchTab {
        id: u32,
        reply: SyncSender<Result<(), String>>,
    },
    /// Close a tab; the reply says whether tabs are left (with none, the window closed).
    CloseTab {
        id: u32,
        reply: SyncSender<Result<bool, String>>,
    },
    /// Close the window, as the person would but without stopping the task: it opens again,
    /// shown or hidden, on its next use.
    Close,
    /// Give a tab (the one acted on with none) a size of its own (sizes.rs), or the whole window.
    Frame(Option<u32>, Option<super::super::sizes::Frame>),
    /// A screenshot of the page, or of a part of it; the reply comes once WebKit has drawn it.
    Capture {
        clip: Option<Clip>,
        reply: SyncSender<Result<Shot, String>>,
        tab: Option<u32>,
    },
    /// Open a panel as a tab: its id, title and page, and what it shares with its caller.
    Panel(panel::Opening),
    /// Run a panel's newest frame.
    Paint(u32),
    /// Close a panel.
    Unpanel(u32),
    /// A panel's menu, where the person right-clicked in it (`m` of its page).
    PanelMenu(u32, (u32, u32), Vec<super::super::choice::Entry>),
    /// A panel's page took a title.
    Titled(u32, String),
    /// What the person clicked on the strip.
    Clicked(String),
    /// What the person clicked in the side: as the strip says it, its points the side's.
    SideClicked(String),
    /// The person clicked in the browser's toolbar: said as the strip says it, its points the
    /// toolbar's.
    NavClicked(String),
    /// The person's settings changed: the window takes them.
    Restyle,
    /// A comment the person made on a page: pictured, then held.
    Noted(Noted),
    /// Tell the strip what the window holds (a tab's title changed, its page loaded).
    Strip,
    /// Integration `.0` was read: on (`.1`) or off. The offer bar is drawn again.
    Offered(&'static str, bool),
    /// The window goes to the project of this key (one the app just made, the person's pick).
    Go(String),
    /// The layout of the windows' projects (`Browser::arranged`), and the windows open, the main
    /// one first: drawn as it is.
    Arranged(Vec<i64>, Vec<Arranged>),
    /// The app opened window `i64` (`Browser::new_window`): the window in use from now on, its
    /// deck made with the first tab it is given.
    NewWindow(i64),
    /// Take in the popups pages opened (popup.rs), and show the last.
    #[cfg_attr(
        all(not(target_os = "macos"), not(test)),
        expect(
            dead_code,
            reason = "elsewhere a page's window opens as a tab of its own"
        )
    )]
    Adopt,
    /// For checks: how the window is dressed, once tinted again.
    Dressed(SyncSender<Option<Dressing>>),
    /// The window's tabs; `None` while it is closed.
    Listing(SyncSender<Option<Vec<WindowTab>>>),
    /// Show or close a tab by its key, for the agent: the task goes on whatever closes.
    Act(Click, SyncSender<Result<(), String>>),
    /// What the agent or the person did to the window's presence (presence.rs).
    Presence(super::super::presence::Event),
    /// The person pressed in one of the app's windows (an NSWindow's address), at `x`, `y`
    /// points from its top left (presses.rs).
    #[cfg_attr(
        all(not(target_os = "macos"), not(test)),
        expect(dead_code, reason = "only macOS tells of presses in its windows")
    )]
    Pressed(usize, f64, f64),
    /// The person opened the app (its Dock icon, `llx-desk open`): the window back, and the app
    /// they use from then on.
    Opened,
    /// For checks: the window's state, `hidden`, `away` or `expanded`.
    Presented(SyncSender<&'static str>),
    /// Ask the person; the reply is whether they went on.
    Confirm(Confirm, SyncSender<bool>),
    Exit(i32),
}

/// How the window is drawn after a command.
#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum Drawn {
    /// It tells of a change (a title, a page loaded, the past kept): drawn once per turn of the
    /// loop, however many came in it.
    Told,
    /// The person or an agent did it, and it may change the tab shown or a tab's project: the
    /// window is kept on the project shown and drawn at once, so the next call sees it. What is
    /// drawn on the way is held until then.
    Settles,
    /// It draws what it changes itself, if anything.
    Itself,
}

/// Each command's one declaration beside the enum: every new command is placed here, the
/// compiler asks for it.
impl Cmd {
    /// Which window it is for: what names a pane goes to the window it is in, a press to its
    /// window; what changes for all is drawn in every one.
    pub(in super::super) fn route(&self) -> Route {
        let web = |id: u32| Route::Pane(Key::Web(id).name());
        let term = |id: u32| Route::Pane(Key::Term(id).name());
        match self {
            Cmd::Eval {
                panel: Some(id), ..
            }
            | Cmd::Paint(id)
            | Cmd::Unpanel(id)
            | Cmd::PanelMenu(id, ..)
            | Cmd::Titled(id, _) => term(*id),
            Cmd::Eval { tab: Some(id), .. }
            | Cmd::Navigate { tab: Some(id), .. }
            | Cmd::Reveal(id)
            | Cmd::SwitchTab { id, .. }
            | Cmd::CloseTab { id, .. }
            | Cmd::Frame(Some(id), _)
            | Cmd::Capture { tab: Some(id), .. } => web(*id),
            Cmd::Act(Click::Show(key) | Click::Close(key), _) => Route::Pane(key.name()),
            Cmd::Pressed(window, ..) => Route::Pressed(*window),
            Cmd::Strip | Cmd::Arranged(..) | Cmd::Restyle | Cmd::Offered(..) => Route::All,
            Cmd::Eval { .. }
            | Cmd::Navigate { .. }
            | Cmd::Frame(..)
            | Cmd::Capture { .. }
            | Cmd::Act(..)
            | Cmd::Open { .. }
            | Cmd::NewTab { .. }
            | Cmd::Close
            | Cmd::Panel(_)
            | Cmd::Clicked(_)
            | Cmd::SideClicked(_)
            | Cmd::NavClicked(_)
            | Cmd::Noted(_)
            | Cmd::Go(_)
            | Cmd::Adopt
            | Cmd::Dressed(_)
            | Cmd::Listing(_)
            | Cmd::Presence(_)
            | Cmd::NewWindow(_)
            | Cmd::Opened
            | Cmd::Presented(_)
            | Cmd::Confirm(..)
            | Cmd::Exit(_) => Route::Current,
        }
    }

    /// How the window is drawn after it.
    pub(in super::super) fn drawn(&self) -> Drawn {
        match self {
            Cmd::Strip | Cmd::Titled(..) => Drawn::Told,
            Cmd::Go(_)
            | Cmd::Arranged(..)
            | Cmd::Panel(_)
            | Cmd::Unpanel(_)
            | Cmd::Act(..)
            | Cmd::Clicked(_)
            | Cmd::SideClicked(_)
            | Cmd::NavClicked(_)
            | Cmd::Close
            | Cmd::NewTab { .. }
            | Cmd::Adopt
            | Cmd::SwitchTab { .. }
            | Cmd::CloseTab { .. } => Drawn::Settles,
            Cmd::Open { .. }
            | Cmd::Eval { .. }
            | Cmd::Navigate { .. }
            | Cmd::Reveal(_)
            | Cmd::Frame(..)
            | Cmd::Capture { .. }
            | Cmd::Paint(_)
            | Cmd::PanelMenu(..)
            | Cmd::Restyle
            | Cmd::Noted(_)
            | Cmd::Offered(..)
            | Cmd::NewWindow(_)
            | Cmd::Dressed(_)
            | Cmd::Listing(_)
            | Cmd::Presence(_)
            | Cmd::Pressed(..)
            | Cmd::Opened
            | Cmd::Presented(_)
            | Cmd::Confirm(..)
            | Cmd::Exit(_) => Drawn::Itself,
        }
    }
}

#[cfg(test)]
#[path = "host_commands_tests.rs"]
mod tests;
