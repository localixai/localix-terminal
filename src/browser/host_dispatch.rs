//! Commands carried out in one native window; every refusal is sent to its caller.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::SyncSender;

use crate::browser::deck::Deck;
use crate::browser::panel::Opening;
use crate::browser::strip::{Click, Key};
use crate::browser::{frame, platform, strip};

use super::super::commands::Drawn;
use super::super::web::on_browser;
use super::super::{Cmd, Window};
use super::{On, act};

impl On<'_> {
    pub(super) fn command(&mut self, command: Cmd) {
        // How the window is drawn after it (`Drawn`).
        let drawn = command.drawn();
        let streamed = if let Cmd::Clicked(said)
        | Cmd::SideClicked(said)
        | Cmd::NavClicked(said)
        | Cmd::DocsClicked(said) = &command
        {
            Self::streamed(said) || crate::browser::tree::said(said).is_some()
        } else {
            false
        };
        // What is drawn on the way (the act, the settle after it) is drawn once, at the end.
        let held = drawn == Drawn::Settles && !streamed;
        if let (true, Some(open)) = (held, self.deck.as_ref()) {
            open.hold();
        }
        self.carry_out(command);
        if drawn == Drawn::Told {
            *self.stale = true;
        } else if held {
            *self.stale = false;
            self.settled();
        }
    }

    /// The window kept on the project shown, its split kept, and what it shows drawn again:
    /// once, however many refreshes keeping it asked for on the way.
    pub(super) fn settled(&mut self) {
        if let Some(open) = self.deck.as_ref() {
            open.hold();
        }
        self.settle();
        self.keep_split();
        if let Some(open) = self.deck.as_ref() {
            open.release();
            open.refresh(self.shared);
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
                    if let Some(gone) = open.end_term(id) {
                        shared.forget(&[Key::Term(gone)]);
                    }
                    open.refresh(shared);
                }
            }
            Cmd::Titled(id, title) => {
                if let Some(open) = self.deck.as_mut() {
                    open.titled(id, title);
                }
            }
            // Drawn once the turn is over (`told`); what waits is said outside the window too.
            Cmd::Strip => {
                crate::browser::alerts::told(&crate::lock(&shared.center), proxy);
                self.announce();
            }
            Cmd::Offered(id, on) => self.offered(id, on),
            Cmd::Go(project) => self.go_project(&project),
            Cmd::Outside(file) => {
                if let Ok(address) = url::Url::from_file_path(&file) {
                    platform::open_outside(address.as_str());
                }
            }
            Cmd::Arranged(_, arranged) => self.arranged(arranged),
            Cmd::Tree(project, news) => {
                if let Some(open) = self.deck.as_ref() {
                    open.tell_tree(self.shared, Some((&project, &news)));
                }
            }
            // Opened by the loop before it got here (`Windows::open`).
            Cmd::NewWindow(_) => {}
            Cmd::Presence(event) => self.presence(event),
            Cmd::Opened => self.opened(),
            Cmd::Presented(reply) => {
                let _ = reply.send(self.presenter.name());
            }
            // Asked on the window, which runs on until the answer comes.
            Cmd::Confirm(question, reply) => self.confirm(&question, reply),
            Cmd::Clicked(said) => self.clicked(&said),
            Cmd::Pressed(window, x, y) => self.pressed(window, x, y),
            // The side's points are the strip's, once moved down by the strip's height: the
            // menus a click there opens are placed in the strip's view.
            Cmd::SideClicked(said) if self.tree_said(&said) => {}
            Cmd::SideClicked(said) => {
                let top = self.deck.as_ref().map_or(0.0, |open| open.top());
                self.clicked(&strip::moved(&said, (0.0, top)));
            }
            Cmd::NavClicked(said) => {
                let at = self.deck.as_ref().map_or((0.0, 0.0), |open| open.nav_at());
                self.clicked(&strip::moved(&said, at));
            }
            Cmd::DocsClicked(said) => {
                let at = self.deck.as_ref().map_or((0.0, 0.0), |open| open.docs_at());
                self.clicked(&strip::moved(&said, at));
            }
            Cmd::Restyle => self.restyled(),
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
                    let pages = open.web.as_ref().map(|tabs| tabs.listed().0);
                    open.end_web(&pages.unwrap_or_default(), shared);
                    open.refresh(shared);
                }
            }
            command @ (Cmd::Eval { .. }
            | Cmd::Navigate { .. }
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
    /// and the project is marked; the window never opens for it.
    fn agent_act(&mut self, said: Click, reply: &SyncSender<Result<(), String>>) {
        if let Click::Show(key) = said
            && self.out_of_sight(&key.name())
        {
            // Its project is not the one shown: the window stays where the person is, and a
            // terminal whose tab they closed is a tab of its project again.
            if let (Key::Term(id), Some(open)) = (key, self.deck.as_mut()) {
                open.unhide_term(id);
                open.refresh(self.shared);
            }
            let _ = reply.send(Ok(()));
            return;
        }
        let _ = reply.send(act(self.deck, said, self.shared));
    }

    /// Terminal `id`'s newest frame, run in its page.
    fn paint(&mut self, id: u32) {
        let open = self.deck.as_ref().and_then(|open| open.term(id));
        if let Some((open, frame)) = open.and_then(|o| Some((o, o.slot.latest()?))) {
            let _ = open.webview.evaluate_script(&frame);
        }
    }

    fn open(&mut self, window: &Window, reply: &SyncSender<Result<(), String>>) {
        let (shared, proxy) = (self.shared, self.proxy);
        let opened = match self.deck.as_mut() {
            Some(open) if open.web.is_some() => Err("the window is already open".into()),
            Some(open) => open.open_web(window, shared, proxy),
            None => Deck::for_web((self.target, self.steps), window, shared, proxy).map(|open| {
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
        // A terminal or a board of a project not shown opens out of sight there; one that opens
        // beside a tab in sight leaves the window to that tab.
        let key = Key::Term(opening.0).name();
        let aside = crate::lock(&self.shared.scope).take_aside(&key);
        let shown = !self.out_of_sight(&key) && !aside && self.deck.is_some();
        let opened = match self.deck.as_mut() {
            Some(open) => open.add_term(opening, shown, self.proxy),
            None => Deck::for_term((self.target, self.steps), opening, self.shared, self.proxy)
                .map(|open| {
                    *self.deck = Some(open);
                }),
        };
        match (opened, self.deck.as_ref()) {
            (Ok(()), Some(open)) => open.refresh(self.shared),
            _ => alive.alive.store(false, Ordering::SeqCst),
        }
    }
}
