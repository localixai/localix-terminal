//! The documents' row (session.rs): the tabs of the files and pages of the session shown, over
//! the pane that shows one of them. It is the strip's own row of tabs in a view of its own, as
//! wide as that pane; with no document in sight it is out of sight.

use std::collections::HashMap;

use tao::dpi::LogicalSize;

use super::super::file_icons;
use super::super::host::Shared;
use super::super::session;
use super::super::strip::{self, DOCS, Key};
use super::super::{Arranged, platform};
use super::{Deck, Part};

impl Deck {
    /// Each tab's documents, and each tab, kept as the layout `arranged` has them now.
    pub(in super::super) fn keep_stacks(&self, arranged: &HashMap<String, Arranged>) {
        *crate::lock(&self.stacks) = session::stacks(arranged.values());
        self.keep_tabs(arranged.values().flat_map(|one| one.tabs.clone()).collect());
    }

    /// The document in sight: the tab shown, else one beside it in a split window.
    pub(super) fn doc_shown(&self) -> Option<Key> {
        self.docs.as_ref()?;
        let sight = self.sight();
        let stacks = crate::lock(&self.stacks);
        session::row(&stacks, self.shown_key(), &sight).map(|row| row.0)
    }

    /// How much of the top of tab `key`'s pane the documents' row takes, in points.
    pub(super) fn docs_over(&self, key: Option<Key>) -> f64 {
        match key.is_some() && self.doc_shown() == key {
            true => DOCS,
            false => 0.0,
        }
    }

    /// The tab shown once tab `key`, shown now, closes: in a split window (or a tab zoomed), the
    /// pane of the tab used last, else the neighbour that takes its share (`taker`); of
    /// documents alone, the one next to it. None leaves it to the window.
    pub(super) fn after_closed(&self, key: Key) -> Option<Key> {
        let mut sight = self.sight();
        // A tab zoomed is drawn whole again without it: the taker is one of all its panes.
        if self.zoomed() == Some(true) {
            sight = (self
                .tab_tree()
                .map(|tree| tree.visible())
                .into_iter()
                .flatten())
            .filter_map(|key| Key::parse(&key))
            .collect();
        }
        if sight.len() >= 2 {
            return self.taker(key, &sight);
        }
        session::next(&crate::lock(&self.stacks), key)
    }

    /// The row laid over the pane of the document in sight: its place in the tree laid out
    /// (`panes`), else the tabs' whole part (`left`, `width` wide, from `top`); out of sight
    /// with none, or while the pages stand aside (`aside`).
    pub(super) fn place_docs(
        &self,
        panes: Option<&super::panes::Panes>,
        (left, width, top): (f64, f64, f64),
        aside: bool,
    ) {
        let Some(docs) = &self.docs else {
            return;
        };
        let shown = self.doc_shown().filter(|_| !aside);
        let spot = shown.map(|doc| {
            (panes.and_then(|panes| panes.rect(&doc)))
                .map_or((left, width, top), |rect| (rect.x, rect.width, rect.y))
        });
        let (x, wide, y) = spot.unwrap_or((-(left + width + 100.0), width, top));
        platform::bound(docs, LogicalSize::new(wide, DOCS), x, y);
    }

    /// Where the row lies in the window, over the pane of the document in sight: what its page
    /// says of a point is from there.
    pub(in super::super) fn docs_at(&self) -> (f64, f64) {
        self.pane_origin(self.doc_shown())
    }
    /// The documents of the row tab `key` is in, in their order, if it is a document.
    pub(in super::super) fn stack_of(&self, key: Key) -> Option<Vec<Key>> {
        let stacks = crate::lock(&self.stacks);
        stacks.iter().find(|stack| stack.contains(&key)).cloned()
    }

    /// The row told the documents it lists and the one in sight.
    pub(super) fn tell_docs(&self) {
        let Some(docs) = &self.docs else {
            return;
        };
        let sight = self.sight();
        let stacks = crate::lock(&self.stacks);
        let row = session::row(&stacks, self.shown_key(), &sight);
        let script = strip::docs(row.map(|(on, keys)| (on, keys.as_slice())));
        self.say((Part::Docs, docs), "pictures", file_icons::script());
        self.say((Part::Docs, docs), "docs", &script);
    }

    /// The documents among `listed` that the lists leave to the row, of the project shown: each
    /// with the tab that stands for its session (session.rs `led`, and `stands_for` for one
    /// the layout has not told of yet).
    pub(in super::super) fn led(
        &self,
        shared: &Shared,
        listed: &[crate::WindowTab],
    ) -> HashMap<String, String> {
        let project = self.shown_project(shared);
        let shown = self.shown_key().map(Key::name);
        let Some(arranged) = shared.arranged_of(&project) else {
            return HashMap::new();
        };
        let mut led = session::led(&arranged, shown.as_deref());
        for tab in Self::project_tabs(listed, &project) {
            if let Some(of) = session::stands_for(Some(&arranged), &tab.id, shown.as_deref()) {
                led.insert(tab.id.clone(), of);
            }
        }
        led
    }
}
