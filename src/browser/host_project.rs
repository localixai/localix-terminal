//! The project the window shows, as the person changes it: the switcher in the bar (the
//! projects the app knows, the shown one checked, one where something happened out of sight
//! marked; Open Folder…; Close Project), and the window kept on the project shown whatever else
//! happens (scope.rs): a tab of another project opened or shown by an agent stays out of sight.

use super::On;
use super::clicks::act;
use crate::browser::choice::Entry;
use crate::browser::strip::{Click, Key};
use crate::browser::{Asked, Heard, agents, icons, platform};

impl On<'_> {
    /// The project's crumb, its menu at `(x, y)` of the strip: Open Folder… first (the one
    /// action among places), then every project the app knows, the one shown checked and one
    /// where something happened out of sight marked; Close Project last.
    pub(super) fn project_menu(&mut self, x: u32, y: u32) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let places = crate::lock(&self.shared.places).clone();
        let scope = crate::lock(&self.shared.scope).clone();
        let item = |label: String, marked: bool, off: bool, icon| Entry::Item {
            label,
            marked,
            off,
            paste: false,
            icon,
        };
        let mut entries = vec![
            item(
                "Open Folder\u{2026}".into(),
                false,
                false,
                Some(icons::FOLDER_PLUS),
            ),
            Entry::Line { line: true },
        ];
        entries.extend(places.iter().map(|place| {
            let label = match scope.marked(&place.key) {
                true => format!("{}  \u{2022}", place.name),
                false => place.name.clone(),
            };
            item(
                label,
                place.key == scope.shown(),
                false,
                Some(icons::FOLDER),
            )
        }));
        entries.push(Entry::Line { line: true });
        let none = scope.shown().is_empty();
        entries.push(item("Close Project".into(), false, none, None));
        let at = (f64::from(x), f64::from(y));
        let Some(picked) = open.pick(at, &entries) else {
            return;
        };
        match picked.checked_sub(1) {
            None => self.open_folder(),
            Some(at) => match places.get(at) {
                Some(place) => self.go_project(&place.key.clone()),
                None => {
                    let shown = scope.shown().to_string();
                    self.close_project(&shown);
                }
            },
        }
    }

    /// The shown tab's crumb, its menu at `(x, y)` of the strip: the project's tabs (and the
    /// window's own), the shown one checked; the one picked is shown.
    pub(super) fn tabs_menu(&mut self, x: u32, y: u32) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let shown = crate::lock(&self.shared.scope).shown().to_string();
        let tabs: Vec<_> = (open.listing(self.shared).into_iter())
            .filter(|tab| tab.group == shown || tab.group.is_empty())
            .filter_map(|tab| Some((Key::parse(&tab.id)?, tab)))
            .collect();
        let entries: Vec<Entry> = (tabs.iter())
            .map(|(key, tab)| {
                let agent = self.shared.offers.agent(&tab.id);
                let mark = agent.and_then(|id| agents::AGENTS.iter().find(|a| a.id == id));
                Entry::Item {
                    label: if tab.title.trim().is_empty() {
                        "New Tab".into()
                    } else {
                        tab.title.clone()
                    },
                    marked: tab.shown,
                    off: false,
                    paste: false,
                    icon: Some(mark.map_or_else(|| icons::of_kind(key.kind()), |agent| agent.mark)),
                }
            })
            .collect();
        let at = (f64::from(x), f64::from(y));
        let picked = open.pick(at, &entries).and_then(|at| tabs.get(at));
        if let Some(&(key, _)) = picked {
            let _ = act(self.deck, Click::Show(key), self.shared);
            if let Some(open) = self.deck.as_ref() {
                open.refresh(self.shared);
            }
        }
    }

    /// The person goes to project `project`: its last tab shown, else, coming from another, the
    /// app opens what it had open (or their shell in its folder). Already there with no tab (the
    /// person closed them), nothing is opened again: what they asked for next opens alone.
    pub(super) fn go_project(&mut self, project: &str) {
        let Some(open) = self.deck.as_ref() else {
            return self.shared.heard(project, Heard::Asked(Asked::Project));
        };
        let own: Vec<String> = (open.listing(self.shared).into_iter())
            .filter(|tab| tab.group == project)
            .map(|tab| tab.id)
            .collect();
        let here = crate::lock(&self.shared.scope).shown() == project;
        if here && own.is_empty() {
            return open.refresh(self.shared);
        }
        let shown = crate::lock(&self.shared.scope).go(project, &own);
        self.shared.heard(project, Heard::Showing);
        match shown.as_deref().and_then(Key::parse) {
            Some(key) => {
                let _ = act(self.deck, Click::Show(key), self.shared);
            }
            None => self.shared.heard(project, Heard::Asked(Asked::Project)),
        }
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
        }
    }

    /// The person picks a folder: the app makes it a project, and the window goes to it.
    pub(super) fn open_folder(&mut self) {
        if let Some(folder) = platform::pick_folder() {
            self.shared.heard("", Heard::Folder(&folder));
        }
    }

    /// Project `project` closed: its tabs; when it was the one shown, the window goes to the
    /// latest project with tabs open, else Home.
    pub(super) fn close_project(&mut self, project: &str) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        // The window's own tabs (the settings) are of no project: nothing to close.
        if project.is_empty() {
            return;
        }
        let project = project.to_string();
        let shown = crate::lock(&self.shared.scope).shown() == project;
        let listed = open.listing(self.shared);
        let own: Vec<Key> = (listed.iter())
            .filter(|tab| tab.group == project)
            .filter_map(|tab| Key::parse(&tab.id))
            .collect();
        for key in own {
            self.close(key);
        }
        if !shown {
            if let Some(open) = self.deck.as_ref() {
                open.refresh(self.shared);
            }
            return;
        }
        let places = crate::lock(&self.shared.places).clone();
        let left = self.deck.as_ref().map(|open| open.listing(self.shared));
        let next = (places.iter())
            .filter(|place| place.key != project)
            .find(|place| (left.iter().flatten()).any(|tab| tab.group == place.key))
            .map(|place| place.key.clone());
        match next {
            Some(next) => self.go_project(&next),
            None => self.shared.heard("", Heard::Asked(Asked::Project)),
        }
    }

    /// The window kept on the project shown: a tab of it shown as the person left it; when the
    /// window came to show another project's tab (one closed, its neighbour shown), the shown
    /// project's own instead, or, with none left, the window follows the tab it shows.
    pub(super) fn settle(&mut self) {
        let Some(open) = self.deck.as_ref() else {
            return;
        };
        let listed = open.listing(self.shared);
        let Some(shown) = listed.iter().find(|tab| tab.shown) else {
            return;
        };
        // A tab of no project (the settings) is the window's own: any project shows it.
        if shown.group.is_empty() {
            return;
        }
        let mut scope = crate::lock(&self.shared.scope);
        if scope.in_sight(&shown.group) {
            if scope.showed(&shown.group, &shown.id) {
                drop(scope);
                self.shared.heard(&shown.group, Heard::Showing);
            }
            return;
        }
        if scope.awaiting() {
            return;
        }
        let project = scope.shown().to_string();
        let own: Vec<String> = (listed.iter())
            .filter(|tab| tab.group == project)
            .map(|tab| tab.id.clone())
            .collect();
        match scope.go(&project, &own).as_deref().and_then(Key::parse) {
            Some(key) => {
                drop(scope);
                let _ = act(self.deck, Click::Show(key), self.shared);
            }
            None => {
                if scope.showed(&shown.group, &shown.id) {
                    drop(scope);
                    self.shared.heard(&shown.group, Heard::Showing);
                }
            }
        }
    }

    /// Whether tab `key` is out of sight, of a project not shown: then that project is marked.
    pub(super) fn out_of_sight(&self, key: &str) -> bool {
        let group = crate::lock(&self.shared.groups).get(key).cloned();
        let Some(group) = group.filter(|group| !group.is_empty()) else {
            return false;
        };
        let mut scope = crate::lock(&self.shared.scope);
        let away = !scope.in_sight(&group);
        scope.touched(&group);
        away
    }
}
