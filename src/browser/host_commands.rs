//! Commands sent to the native event loop; no window or shared state lives here.

use std::sync::mpsc::SyncSender;

use crate::WindowTab;

use super::super::strip::Click;
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
    /// Load an address: in this tab, else the one acted on.
    Navigate(String, Option<u32>),
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
        not(target_os = "macos"),
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
