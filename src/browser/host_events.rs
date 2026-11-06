//! The event loop: it takes the main thread, opens the window when asked, and carries out each
//! [`Cmd`]: the window's own, a panel's, and the browser's on its tabs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use std::sync::mpsc::SyncSender;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};

use super::super::comments::Comments;
use super::super::deck::{Deck, Shown};
use super::super::frame;
use super::super::guard::token;
use super::super::panel::Opening;
use super::super::placement::Kept;
use super::super::platform;
use super::super::presence::Event as Presence;
use super::super::strip::{self, Click, Key};
use super::presence::{Presenter, Stage};
use super::web::on_browser;
use super::window::on_window;
use super::{Browser, Cmd, Heard, Shared, Window};

/// Take the main thread for the browser's event loop and run `work` on a worker thread with a
/// handle that opens the window when it is needed. The event loop never returns, so the process
/// exits here, with the code `work` returns (the `i32` is the type the headless build's `host`
/// shares, which does return).
pub fn host<F>(work: F) -> i32
where
    F: FnOnce(Browser) -> i32 + Send + 'static,
{
    // Changed on macOS only.
    #[cfg_attr(
        not(target_os = "macos"),
        expect(unused_mut, reason = "only macOS changes the loop before it runs")
    )]
    let mut event_loop = EventLoopBuilder::<Cmd>::with_user_event().build();
    // Nothing the agent does takes the keyboard: the person keeps typing where they were. The app
    // starts in the background (`platform::dock` makes it a regular one when it is the app or
    // shows a window), and only the person's own acts make it the active app.
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activate_ignoring_other_apps(false);
        event_loop.set_activation_policy(ActivationPolicy::Prohibited);
    }
    let proxy = event_loop.create_proxy();
    let shared = fresh();
    let browser = Browser {
        proxy: proxy.clone(),
        shared: Arc::clone(&shared),
    };
    let (exit, refused) = (proxy.clone(), proxy.clone());
    let worker = std::thread::Builder::new().name("desk-work".into());
    let started = worker.spawn(move || {
        let code = work(browser);
        let _ = exit.send_event(Cmd::Exit(code));
    });
    if started.is_err() {
        // No thread for the work: the process ends at once, as a failure (1, as anywhere).
        let _ = refused.send_event(Cmd::Exit(1));
    }

    platform::watch_spaces(&proxy);
    let mut deck: Option<Deck> = None;
    let mut presenter = Presenter::new();
    let mut docked = false;
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "tao's events: foreign, non-exhaustive"
        )]
        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                platform::dock(target, false, &proxy);
                platform::quit_through(&proxy);
            }
            // The person opened the app again (its Dock icon, the Finder): the desk gives them a
            // window, with their shell when it has no tab.
            Event::Reopen { .. } => shared.heard("", Heard::Reopened),
            #[expect(
                clippy::exit,
                clippy::disallowed_methods,
                reason = "the event loop owns the main thread and never returns: the process ends \
                          here, with the code the run returned (main.rs hands it over)"
            )]
            Event::UserEvent(Cmd::Exit(code)) => std::process::exit(code),
            Event::UserEvent(command) => {
                let mut on = On {
                    deck: &mut deck,
                    presenter: &mut presenter,
                    target,
                    shared: &shared,
                    proxy: &proxy,
                };
                on.command(command);
            }
            Event::WindowEvent {
                window_id, event, ..
            } => on_window((&mut deck, &presenter), window_id, &event, &shared),
            _ => {}
        }
        let mut stage = Stage {
            deck: &mut deck,
            target,
            shared: &shared,
            proxy: &proxy,
        };
        presenter.sync(&mut stage);
        let shown = deck.as_ref().is_some_and(Deck::is_shown);
        if shown != docked {
            docked = shown;
            platform::dock(target, shown, &proxy);
        }
    })
}

/// What the loop and the worker share, as it starts.
fn fresh() -> Arc<Shared> {
    Arc::new(Shared {
        token: token(),
        open: AtomicBool::new(false),
        next_id: AtomicU64::new(1),
        pending: Mutex::new(HashMap::new()),
        loads: Mutex::new(HashMap::new()),
        loaded: Condvar::new(),
        active: AtomicU32::new(1),
        seq: AtomicU64::new(0),
        tabs: Mutex::new(vec![]),
        listener: Mutex::new(None),
        groups: Mutex::new(HashMap::new()),
        places: Mutex::default(),
        scope: Mutex::default(),
        details: Mutex::new(HashMap::new()),
        settings: Mutex::new(None),
        undrawn: Mutex::default(),
        center: Mutex::default(),
        ledgers: Mutex::default(),
        comments: Comments::default(),
        turn: Default::default(),
        notices: Default::default(),
        kept: Mutex::new(Kept::default()),
        offers: crate::browser::offer::Offers::default(),
        folders: Mutex::default(),
    })
}

#[path = "host_clicks.rs"]
mod clicks;
#[path = "host_offer.rs"]
mod offer_bar;
#[path = "host_project.rs"]
mod project;
#[path = "host_settings.rs"]
mod settings_tab;
#[path = "host_tab_menu.rs"]
mod tab_menu;
use clicks::act;

/// What a command is carried out with, on the event loop.
struct On<'a> {
    deck: &'a mut Option<Deck>,
    presenter: &'a mut Presenter,
    target: &'a EventLoopWindowTarget<Cmd>,
    shared: &'a Arc<Shared>,
    proxy: &'a EventLoopProxy<Cmd>,
}

impl On<'_> {
    fn command(&mut self, command: Cmd) {
        let shared = self.shared;
        // What may change the tab shown, or a tab's project: after it, the window is kept on the
        // project shown, and the strip drawn.
        let settles = matches!(
            command,
            Cmd::Strip
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
                | Cmd::CloseTab { .. }
        );
        self.carry_out(command);
        if settles {
            self.settle();
            if let Some(open) = self.deck.as_ref() {
                open.refresh(shared);
            }
        }
    }

    fn carry_out(&mut self, command: Cmd) {
        let (shared, proxy) = (self.shared, self.proxy);
        match command {
            Cmd::Open { window, reply } => self.open(&window, &reply),
            Cmd::Panel(opening) => self.panel(opening),
            Cmd::Paint(id) => self.paint(id),
            Cmd::PanelMenu(id, at, items) => self.panel_menu(id, at, &items),
            Cmd::Reveal(id) => {
                let web = self.deck.as_ref().and_then(|deck| deck.web.as_ref());
                if let Some(webview) = web.and_then(|tabs| tabs.at(Some(id))) {
                    platform::see_through(webview, false);
                }
                // The tab draws now: the strip says so.
                let _ = self.proxy.send_event(Cmd::Strip);
            }
            Cmd::Unpanel(id) => {
                if let Some(open) = self.deck.as_mut() {
                    match open.end_term(id) {
                        true => open.refresh(shared),
                        false => *self.deck = None,
                    }
                }
            }
            Cmd::Titled(id, title) => {
                if let Some(open) = self.deck.as_mut() {
                    open.titled(id, title);
                    open.refresh(shared);
                }
            }
            Cmd::Strip => self.renamed(),
            Cmd::Offered(id, on) => self.offered(id, on),
            Cmd::Go(project) => self.go_project(&project),
            Cmd::Presence(event) => self.presence(event),
            Cmd::Opened => self.opened(),
            Cmd::Pictured(shot) => self.pictured(&shot),
            Cmd::Presented(reply) => {
                let _ = reply.send(self.presenter.name());
            }
            Cmd::Confirm(question, reply) => {
                let _ = reply.send(platform::confirm(&question));
            }
            Cmd::Clicked(said) => self.clicked(&said),
            // The side's points are the strip's, once moved down by the strip's height: the
            // menus a click there opens are placed in the strip's view.
            Cmd::SideClicked(said) => {
                let top = self.deck.as_ref().map_or(0.0, |open| open.top());
                self.clicked(&strip::moved(&said, (0.0, top)));
            }
            Cmd::NavClicked(said) => {
                let at = self.deck.as_ref().map_or((0.0, 0.0), |open| open.nav_at());
                self.clicked(&strip::moved(&said, at));
            }
            Cmd::Restyle => self.restyled(),
            Cmd::Slide(slide, now) => {
                if let Some(open) = self.deck.as_mut() {
                    open.slid(slide, now, shared);
                }
            }
            Cmd::Act(said, reply) => self.agent_act(said, &reply),
            Cmd::Listing(reply) => {
                let _ = reply.send(self.deck.as_ref().map(|open| open.listing(shared)));
            }
            Cmd::Dressed(reply) => {
                let _ = reply.send(self.deck.as_ref().map(|open| {
                    open.frame.retint();
                    frame::dressed(&open.frame.native)
                }));
            }
            Cmd::Eval {
                id,
                script,
                panel: Some(panel),
                ..
            } => self.eval_panel(id, &script, panel),
            Cmd::Close => {
                // The agent closed the browser: the terminals stay.
                if let Some(open) = self.deck.as_mut() {
                    shared.closed();
                    match open.remove_web() {
                        true => open.refresh(shared),
                        false => *self.deck = None,
                    }
                }
            }
            command @ (Cmd::Eval { .. }
            | Cmd::Navigate(..)
            | Cmd::NewTab { .. }
            | Cmd::Adopt
            | Cmd::SwitchTab { .. }
            | Cmd::CloseTab { .. }
            | Cmd::Frame(..)
            | Cmd::Capture { .. }
            | Cmd::Noted(_)
            | Cmd::Exit(_)) => on_browser(command, self.deck, shared, proxy),
        }
    }

    /// The agent shows or closes a tab. One of a project not shown stays out of sight there,
    /// and the project is marked; the mini blinks as for any, and never opens.
    fn agent_act(&mut self, said: Click, reply: &SyncSender<Result<(), String>>) {
        if let Click::Show(key) = said
            && self.out_of_sight(&key.name())
        {
            self.presence(Presence::Show);
            let _ = reply.send(Ok(()));
            return;
        }
        let done = act(self.deck, said, self.shared);
        if done.is_ok() && matches!(said, Click::Show(_)) {
            self.presence(Presence::Show);
        }
        let _ = reply.send(done);
    }

    /// Terminal `id`'s newest frame, run in its page. The mini pictures the terminal shown as it
    /// draws, not only when it is shown: its first picture may come before its first frame.
    fn paint(&mut self, id: u32) {
        let open = self.deck.as_ref().and_then(|open| open.term(id));
        if let Some((open, frame)) = open.and_then(|o| Some((o, o.slot.latest()?))) {
            let _ = open.webview.evaluate_script(&frame);
        }
        if self
            .deck
            .as_ref()
            .is_some_and(|deck| deck.shown == Shown::Term(id))
        {
            self.presence(Presence::Changed);
        }
    }

    fn open(&mut self, window: &Window, reply: &SyncSender<Result<(), String>>) {
        let (shared, proxy) = (self.shared, self.proxy);
        let opened = match self.deck.as_mut() {
            Some(open) if open.web.is_some() => Err("the window is already open".into()),
            Some(open) => open.open_web(window, shared, proxy),
            None => Deck::for_web(self.target, window, shared, proxy).map(|open| {
                *self.deck = Some(open);
            }),
        };
        // Whether the window is open now: a second open refused leaves the first one open.
        let web = self.deck.as_ref().is_some_and(|open| open.web.is_some());
        shared.open.store(web, Ordering::SeqCst);
        let _ = reply.send(opened.clone());
        if let (Ok(()), Some(open)) = (opened, self.deck.as_ref()) {
            open.refresh(shared);
        }
    }

    fn panel(&mut self, opening: Opening) {
        let alive = Arc::clone(&opening.3);
        // A terminal or a board of a project not shown opens out of sight there.
        let shown = !self.out_of_sight(&Key::Term(opening.0).name());
        let opened = match self.deck.as_mut() {
            Some(open) => open.add_term(opening, shown, self.proxy),
            None => Deck::for_term(self.target, opening, self.shared, self.proxy).map(|open| {
                *self.deck = Some(open);
            }),
        };
        match (opened, self.deck.as_ref()) {
            (Ok(()), Some(open)) => open.refresh(self.shared),
            _ => alive.alive.store(false, Ordering::SeqCst),
        }
    }
}
