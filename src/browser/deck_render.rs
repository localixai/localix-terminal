//! The deck's current state rendered as data for its strip, side and page toolbar.

use super::super::host::Shared;
use super::super::panel::Held;
use super::super::strip::{self, Item, Key};
use super::{Deck, Menu, Part, Shown};
use crate::WindowTab;

impl Deck {
    /// Every tab as the strip shows them: a project's together, the browser's first in each; its
    /// key, its kind, its title, what it holds and its group.
    pub(crate) fn listing(&self, shared: &Shared) -> Vec<WindowTab> {
        let groups = crate::lock(&shared.groups).clone();
        let group = |key: Key| groups.get(&key.name()).cloned().unwrap_or_default();
        let details = crate::lock(&shared.details).clone();
        let mut out = vec![];
        if let Some(web) = &self.web {
            let (ids, active) = web.listed();
            let known = crate::lock(&shared.tabs).clone();
            for id in ids {
                let tab = known.iter().find(|tab| tab.id == id);
                let (title, url) = tab.map_or((String::new(), String::new()), |tab| {
                    (tab.title.trim().to_string(), tab.url.clone())
                });
                out.push(WindowTab {
                    id: Key::Web(id).name(),
                    kind: "browser".into(),
                    title: if title.is_empty() { url.clone() } else { title },
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
                kind: if key.kind() == "board" {
                    "board"
                } else {
                    "terminal"
                }
                .into(),
                title: title.to_string(),
                detail: details.get(&key.name()).cloned().unwrap_or_default(),
                shown: self.shown == Shown::Term(open.id),
                acted_on: false,
                group: group(Key::Term(open.id)),
            });
        }
        // In the layout's order (each tab's panes left to right), the rest as they came; each
        // project's together.
        let arranged = shared.arranged_here();
        let place = |tab: &WindowTab| {
            (arranged.get(&tab.group).into_iter())
                .flat_map(|arranged| arranged.tabs.iter().flat_map(|tab| tab.panes.iter()))
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
        let listed = self.listing(shared);
        let shown_key = (listed.iter().find(|tab| tab.shown)).and_then(|tab| Key::parse(&tab.id));
        let bar = shown_key.and_then(|key| shared.offers.bar(key, &self.proxy));
        crate::lock(&self.offer).clone_from(&bar);
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
        let items: Vec<Item> = listed
            .into_iter()
            .filter_map(|tab| {
                Some(Item {
                    key: Key::parse(&tab.id)?,
                    title: tab.title,
                    detail: tab.detail,
                    agent: shared.offers.agent(&tab.id),
                    group: tab.group,
                })
            })
            .collect();
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
        ];
        for (part, view) in parts {
            let Some(view) = view else { continue };
            self.say((part, view), "split", &split);
            self.say((part, view), "agents", &super::super::agents::script());
            self.say((part, view), "drawn", &drawn);
        }
        self.tell_frame();
        self.tell_menus(strip, shared);
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
        let (acting, waiting) = shared.turn.now();
        let turn = strip::Turn::of(acting, self.commenting, waiting);
        self.say((Part::Nav, nav), "turn", &strip::turn(turn));
    }

    /// The strip's open list: the history and the Event Center draw into it, each saying what it
    /// shows. While the Events show, the history's last word is drawn over, so it is said again
    /// after; shut, the Event Center draws nothing, and its events are read only while it is open.
    fn tell_menus(&self, strip: &wry::WebView, shared: &Shared) {
        let rows = (self.menu == Some(Menu::History)).then(|| self.rows(shared));
        if self.menu == Some(Menu::Events) {
            crate::lock(&self.said).forget(&(Some(Part::Strip), "menu"));
        } else {
            self.say((Part::Strip, strip), "menu", &strip::menu(rows.as_deref()));
        }
        let center = match self.menu == Some(Menu::Events) {
            true => strip::center(&crate::lock(&shared.center), true),
            false => strip::center("", false),
        };
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
        ];
        for (part, view) in views {
            if let Some(view) = view {
                self.say((part, view), "frame", &script);
            }
        }
    }

    /// `script`, of `kind`, run in `view`, which is `part` of the strip, unless it is what that
    /// part was last told of that kind (and no part has loaded its page again since): a refresh
    /// with nothing new wakes no page. A part is told apart by what it is, not where its view
    /// lives: windows come and go, and a deck moves.
    fn say(&self, (part, view): (Part, &wry::WebView), kind: &'static str, script: &str) {
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
