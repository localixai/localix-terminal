//! The deck's current state rendered as data for its strip, side and page toolbar.

use super::super::host::Shared;
use super::super::panel::Held;
use super::super::session;
use super::super::strip::{self, Item, Key};
use super::super::{agents, alerts};
use super::{Deck, Menu, Part, Shown};
use crate::WindowTab;

impl Deck {
    /// Every tab as the strip shows them: a project's together, the browser's first in each; its
    /// key, its kind, its title, what it holds and its group.
    pub(crate) fn listing(&self, shared: &Shared) -> Vec<WindowTab> {
        self.listed(shared, &shared.arranged_here())
    }

    /// The tabs as `listing` gives them, in the order of the layout `arranged`.
    fn listed(
        &self,
        shared: &Shared,
        arranged: &std::collections::HashMap<String, super::super::Arranged>,
    ) -> Vec<WindowTab> {
        let groups = crate::lock(&shared.groups).clone();
        let group = |key: Key| groups.get(&key.name()).cloned().unwrap_or_default();
        let details = crate::lock(&shared.details).clone();
        let mut out = vec![];
        if let Some(web) = &self.web {
            let (ids, active) = web.listed();
            let known = crate::lock(&shared.tabs).clone();
            for id in ids {
                let tab = known.iter().find(|tab| tab.id == id);
                let (title, url) =
                    tab.map_or_else(Default::default, |tab| (tab.name(), tab.url.clone()));
                out.push(WindowTab {
                    id: Key::Web(id).name(),
                    kind: "browser".into(),
                    title,
                    detail: url,
                    shown: self.shown == Shown::Web && id == active,
                    acted_on: id == active,
                    group: group(Key::Web(id)),
                });
            }
        }
        for open in self.terms.iter().filter(|open| open.held == Held::Live) {
            let title = open.title.strip_prefix("desk · ").unwrap_or(&open.title);
            let key = Key::Term(open.id);
            out.push(WindowTab {
                id: key.name(),
                kind: match key.kind() {
                    "term" | "settings" => "terminal",
                    kind => kind,
                }
                .into(),
                title: title.to_string(),
                detail: details.get(&key.name()).cloned().unwrap_or_default(),
                shown: self.shown == Shown::Term(open.id),
                acted_on: false,
                group: group(Key::Term(open.id)),
            });
        }
        // In the layout's order (each tab's panes left to right, a session's documents as it
        // keeps them), the rest as they came; each project's together.
        let place = |tab: &WindowTab| {
            (arranged.get(&tab.group).into_iter())
                .flat_map(|arranged| arranged.tabs.iter().flat_map(|tab| tab.all.iter()))
                .position(|key| *key == tab.id)
                .unwrap_or(usize::MAX)
        };
        out.sort_by_cached_key(place);
        strip::by_group(out, |tab| tab.group.as_str())
    }

    /// Refreshes held from now on: none is drawn until `release`, and the one after it draws all.
    pub(in super::super) fn hold(&self) {
        self.held.hold();
    }

    /// Refreshes drawn as they are asked for again.
    pub(in super::super) fn release(&self) {
        self.held.release();
    }

    /// Lay the window out again and tell the strip what it holds now; while refreshes are held
    /// (`hold`), once they are let go.
    pub(in super::super) fn refresh(&self, shared: &Shared) {
        if !self.held.now() {
            return;
        }
        let arranged = shared.arranged_here();
        let listed = self.listed(shared, &arranged);
        let shown_key = (listed.iter().find(|tab| tab.shown)).and_then(|tab| Key::parse(&tab.id));
        let bar = shown_key.and_then(|key| shared.offers.bar(key, &self.proxy));
        crate::lock(&self.offer).clone_from(&bar);
        self.keep_stacks(&arranged);
        self.layout();
        // The page shown comments as the strip says: told again once another tab is shown, or
        // its page loaded again (its load finished counts it).
        if let Some(web) = &self.web {
            let id = web.listed().1;
            let loaded = (crate::lock(&shared.loads).get(&id)).map_or(0, |loads| loads.finished);
            let script = strip::comment(&shared.key, self.commenting, self.sent);
            self.tell((None, web.active()), "comment", &script, (id, loaded));
        }
        let Some(strip) = &self.strip else {
            return;
        };
        self.say(
            (Part::Strip, strip),
            "offer",
            &super::super::offer::script(bar.as_ref()),
        );
        let shown = listed
            .iter()
            .find(|tab| tab.shown)
            .map(|tab| tab.id.clone());
        let items = Self::items(listed, &arranged, shown.as_deref(), shared);
        let shown = shown.and_then(|id| Key::parse(&id)).unwrap_or(Key::Web(0));
        let side = self.side.as_ref().map(|_| self.side_open);
        let places = crate::lock(&shared.places).clone();
        let scope = crate::lock(&shared.scope).clone();
        let project = self.shown_project(shared);
        let projects = strip::Projects {
            shown: &project,
            places: (places.iter())
                .map(|place| (place, scope.marked(&place.key)))
                .collect(),
        };
        let drawn = strip::script(&items, shown, side, &projects);
        // The split's two tabs, left and right: the side lists them as one; another tab shown,
        // the project's split still listed so.
        let on = self.pair().map(|(left, right)| [left.name(), right.name()]);
        let kept = if self.split.told == super::super::split::Told::Whole {
            None
        } else {
            (shared.arranged_of(&project).as_ref())
                .and_then(super::super::split::kept_of)
                .as_ref()
                .map(super::super::split::sides)
        };
        let split = format!("SP({})", serde_json::json!(on.or(kept)));
        let parts = [
            (Part::Strip, Some(strip)),
            (Part::Side, self.side.as_ref()),
            (Part::Nav, self.nav.as_ref()),
            (Part::Docs, self.docs.as_ref()),
        ];
        for (part, view) in parts {
            let Some(view) = view else { continue };
            self.say((part, view), "split", &split);
            self.say((part, view), "drawn", &drawn);
        }
        self.tell_docs();
        self.tell_frame();
        self.tell_tree(shared, None);
        self.tell_menus(strip, shared);
        self.tell_dialog(strip);
        let framed = (self.web.as_ref())
            .filter(|_| self.shown == Shown::Web)
            .and_then(|web| web.frame(None));
        // The page's size, comments and whose turn it is: the browser's toolbar's.
        let Some(nav) = &self.nav else {
            return;
        };
        let visited = strip::visited(&self.visited(shared));
        self.say((Part::Nav, nav), "visited", &visited);
        let (size, _, top) = self.content();
        let scale = super::super::sizes::place(size, top, framed).scale;
        self.say(
            (Part::Nav, nav),
            "sizes",
            &super::super::sizes::script(framed, scale),
        );
        let held = shared.comments.count();
        self.say(
            (Part::Nav, nav),
            "commenting",
            &strip::commenting(self.commenting, held),
        );
        self.say_turn(shared, nav);
    }

    /// The tabs `listed` as the strip's pages list them, tab `shown` the one the window shows:
    /// each with what waits in it, its agent, and, a document, the tab that stands for its
    /// session (the lists show the session; the document is a tab of the row over it), from
    /// the moment it opens.
    fn items(
        listed: Vec<WindowTab>,
        arranged: &std::collections::HashMap<String, super::super::Arranged>,
        shown: Option<&str>,
        shared: &Shared,
    ) -> Vec<Item> {
        let mut waits = alerts::waits(&alerts::read(&crate::lock(&shared.center)));
        let mut led: std::collections::HashMap<String, String> = (arranged.values())
            .flat_map(|arranged| session::led(arranged, shown))
            .collect();
        (listed.into_iter())
            .filter_map(|tab| {
                // A document just opened is in no tab of the layout yet: it is no row meanwhile.
                let of = led
                    .remove(&tab.id)
                    .or_else(|| session::stands_for(arranged.get(&tab.group), &tab.id, shown));
                Some(Item {
                    key: Key::parse(&tab.id)?,
                    title: tab.title,
                    detail: tab.detail,
                    agent: shared.offers.agent(&tab.id),
                    waits: waits.remove(&tab.id),
                    of,
                    group: tab.group,
                })
            })
            .collect()
    }

    /// The dialog up (dialog.rs), drawn over the whole window; one the person asked for takes
    /// the keys.
    fn tell_dialog(&self, strip: &wry::WebView) {
        let dialog = super::super::dialog::shown();
        let drawn = super::super::dialog::script(dialog.as_ref());
        self.say((Part::Strip, strip), "dialog", &drawn);
        if dialog.is_some_and(|dialog| dialog.asked) && self.frame.is_key() {
            let _ = strip.focus();
        }
    }

    /// The strip's open list: the history and the Event Center draw into it, each saying what it
    /// shows. While the Events show, the history's last word is drawn over, so it is said again
    /// after. Shut, the Event Center still hears its events: its button's dot says what is new.
    fn tell_menus(&self, strip: &wry::WebView, shared: &Shared) {
        let rows = (self.menu == Some(Menu::History)).then(|| self.rows(shared));
        if self.menu == Some(Menu::Events) {
            crate::lock(&self.said).forget(&(Some(Part::Strip), "menu"));
        } else {
            self.say((Part::Strip, strip), "menu", &strip::menu(rows.as_deref()));
        }
        let open = self.menu == Some(Menu::Events);
        let center = strip::center(&crate::lock(&shared.center), open);
        self.say((Part::Strip, strip), "center", &center);
    }

    /// The pages know the window's state: full screen (`data-full`: it has no buttons of its own
    /// then, and the crumbs give up the room kept for them) and inactive (`data-inactive`: another
    /// window is key, and the chrome recedes). Told on every refresh too, so a page loaded again
    /// has it back; WebKit's `:window-inactive` never restyles what it has drawn.
    pub(in super::super) fn tell_frame(&self) {
        let full = self.frame.native.fullscreen().is_some();
        let inactive = !self.frame.is_key();
        let script = format!(
            "document.documentElement.toggleAttribute('data-full',{full});\
             document.documentElement.toggleAttribute('data-inactive',{inactive})"
        );
        let views = [
            (Part::Strip, self.strip.as_ref()),
            (Part::Side, self.side.as_ref()),
            (Part::Nav, self.nav.as_ref()),
            (Part::Docs, self.docs.as_ref()),
        ];
        for (part, view) in views {
            if let Some(view) = view {
                self.say((part, view), "frame", &script);
            }
        }
    }

    /// Whose turn it is, and the agents installed (their marks where no tab is left), said to
    /// the navigation's view.
    fn say_turn(&self, shared: &Shared, nav: &wry::WebView) {
        let (acting, waiting) = shared.turn.now();
        let turn = strip::Turn::of(acting, self.commenting, waiting);
        self.say((Part::Nav, nav), "turn", &strip::turn(turn));
        let starts = strip::starts(&agents::listed());
        self.say((Part::Nav, nav), "starts", &starts);
    }

    /// `script`, of `kind`, run in `view`, which is `part` of the strip, unless it is what that
    /// part was last told of that kind (and no part has loaded its page again since): a refresh
    /// with nothing new wakes no page. A part is told apart by what it is, not where its view
    /// lives: windows come and go, and a deck moves.
    pub(super) fn say(
        &self,
        (part, view): (Part, &wry::WebView),
        kind: &'static str,
        script: &str,
    ) {
        self.tell((Some(part), view), kind, script, ());
    }

    /// `script` run in `view` (a part of the strip, or none: the page shown) unless it is what
    /// that view was last told of `kind` for the same `about` (a page's own load).
    fn tell(
        &self,
        (part, view): (Option<Part>, &wry::WebView),
        kind: &'static str,
        script: &str,
        about: impl std::hash::Hash,
    ) {
        let loads = super::LOADS.load(std::sync::atomic::Ordering::SeqCst);
        let news = crate::lock(&self.said).news(loads, (part, kind), script, about);
        if news {
            let _ = view.evaluate_script(script);
        }
    }
}
