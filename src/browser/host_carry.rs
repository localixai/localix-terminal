//! A tab carried from a side's list, as the loop carries it out (`carry.rs` says what each of
//! the page's words does in each state): its card drawn at the pointer (`chip.rs`); over its
//! window's panes, the tab standing where it would land (`deck_preview.rs`), aimed by the
//! zones taken from the panes as it came over them, and let go there into the layout; past the
//! window's edge, out into a window of its own (`host_tear.rs`). One state for the whole
//! gesture, kept by the loop, since the windows share it.

use std::time::Instant;

use tao::event_loop::EventLoopWindowTarget;

use super::super::super::carry::{self, Do, Phase, Point, Said};
use super::super::super::carry_zones::Zones;
use super::super::super::chip::Chip;
use super::super::super::deck::Plan;
use super::super::super::strip::Key;
use super::tear::Tear;
use super::{Cmd, Looping};

/// How far past the window's edge the pointer carries a tab before it comes out (points).
const OUT: f64 = 10.0;

/// The tab carried, while it is.
pub(super) struct Carrying {
    /// Its pane; none for an entry of the files' tree, which is only drawn at the pointer.
    key: Option<Key>,
    /// The window it was lifted in.
    pub(super) from: usize,
    pub(super) chip: Chip,
    phase: Phase,
    /// The zones of its window's panes while it is over them.
    zones: Option<Zones<Key, Plan>>,
    /// The window it is carried in once out of its own.
    pub(super) tear: Option<Tear>,
}

impl Looping {
    /// A side's word, when it is about a carried tab: whether it was, and is done with.
    pub(super) fn carried(&mut self, message: &str, target: &EventLoopWindowTarget<Cmd>) -> bool {
        let Some(said) = carry::said(message) else {
            return false;
        };
        let now = self.carry.as_ref().map(|carrying| carrying.phase);
        let (then, does) = carry::next(now, &said, self.past(&said));
        match (does, &said) {
            (Do::Lift, _) => self.lift(&said),
            (Do::Follow, Said::At((x, y))) => {
                if let Some(carrying) = &self.carry {
                    carrying.chip.follow(*x, *y);
                }
            }
            (Do::Enter, Said::Over { at, .. }) => self.enter(*at),
            (Do::Aim, Said::Over { at, .. }) => self.aim(*at),
            (Do::Leave, _) => self.leave(),
            (Do::ComeOut | Do::Move, Said::Over { key, at, screen }) => {
                if does == Do::ComeOut {
                    self.leave();
                }
                self.follow(*key, *at, *screen, target);
            }
            (Do::Land, Said::Drop { at, .. }) => self.land(*at, target),
            (Do::LetGo, _) => drop(self.let_go()),
            (Do::TakeBack, _) => drop(self.take_back()),
            (Do::Slot, Said::Slot(before)) => {
                if let Some(tear) = self.carry.as_mut().and_then(|it| it.tear.as_mut()) {
                    tear.before = before.map(str::to_string);
                }
            }
            _ => {}
        }
        match then {
            Some(phase) => {
                if let Some(carrying) = self.carry.as_mut() {
                    carrying.phase = phase;
                }
            }
            None => {
                if let Some(carrying) = self.carry.take() {
                    self.stand_nowhere(carrying.from, carrying.key);
                }
            }
        }
        true
    }

    /// Tab `key` stands nowhere over the panes of window `from`: they lie as the layout keeps
    /// them, laid out again only when a landing was shown.
    fn stand_nowhere(&mut self, from: usize, key: Option<Key>) {
        if let (Some(key), Some(deck)) = (key, self.windows.deck_mut(from)) {
            deck.stand(key, None);
        }
    }

    /// Whether `said` puts the pointer past the edge of the window the tab was lifted in.
    fn past(&self, said: &Said<'_>) -> bool {
        let (Said::Over { at, .. }, Some(carrying)) = (said, &self.carry) else {
            return false;
        };
        let Some(deck) = self.windows.deck(carrying.from) else {
            return false;
        };
        let Some((.., wide, tall)) = deck.on_screen() else {
            return false;
        };
        let (x, y) = (at.0, at.1 + deck.top());
        x < -OUT || y < -OUT || x > wide + OUT || y > tall + OUT
    }

    /// A tab held in the window in use's side: its card drawn where the pointer is from now on,
    /// over everything, till it is let go, and its window made ready for it to land.
    fn lift(&mut self, said: &Said<'_>) {
        let Said::Lift {
            key,
            grab,
            rect,
            inner,
        } = said
        else {
            return;
        };
        if let Some(was) = self.carry.take() {
            self.stand_nowhere(was.from, was.key);
        }
        let from = self.windows.at();
        let Some(deck) = self.windows.deck_mut(from) else {
            return;
        };
        let Some(side) = deck.side_view() else {
            return;
        };
        let chip = Chip::lift(side, *rect, *grab, *inner);
        if key.is_some() {
            deck.ready_to_land();
        }
        self.carry = Some(Carrying {
            key: *key,
            from,
            chip,
            phase: Phase::Lifted,
            zones: None,
            tear: None,
        });
    }

    /// The tab came over its window's panes: their zones taken as they lie now, then aimed.
    fn enter(&mut self, at: Point) {
        let Some(carrying) = self.carry.as_mut() else {
            return;
        };
        let Some(deck) = self.windows.deck(carrying.from) else {
            return;
        };
        carrying.zones = Some(deck.zones());
        self.aim(at);
    }

    /// The pointer at `at` of the side's page, over the panes: where the tab lands looked for,
    /// and the window laid out for it only when that changed.
    fn aim(&mut self, at: Point) {
        let Some(carrying) = self.carry.as_mut() else {
            return;
        };
        let (Some(key), Some(zones)) = (carrying.key, carrying.zones.as_mut()) else {
            return;
        };
        let Some(deck) = self.windows.deck_mut(carrying.from) else {
            return;
        };
        let started = Instant::now();
        if zones.at(at.0, at.1 + deck.top()) {
            let plan = zones.plan(|beside, side| deck.plan(key, beside, side));
            deck.stand(key, plan.map(|plan| (&plan.laid, plan.landed.side)));
        }
        deck.aimed(started.elapsed());
    }

    /// The tab off the panes, or out past its window's edge: its zones let go of, and the
    /// panes back where the layout keeps them.
    fn leave(&mut self) {
        let Some(carrying) = self.carry.as_mut() else {
            return;
        };
        carrying.zones = None;
        let (from, key) = (carrying.from, carrying.key);
        self.stand_nowhere(from, key);
    }

    /// The tab let go over the panes at `at`: where it stands told to the layout, which the
    /// window is laid out from again, once, each terminal told the grid that fits its pane
    /// (`On::land`); let go where nothing changes, the panes go back.
    fn land(&mut self, at: Point, target: &EventLoopWindowTarget<Cmd>) {
        self.aim(at);
        let Some(carrying) = self.carry.as_mut() else {
            return;
        };
        let (key, from) = (carrying.key, carrying.from);
        let zones = carrying.zones.take();
        let landed = zones.and_then(|mut zones| Some(zones.plan(|_, _| None)?.landed.clone()));
        let (Some(key), Some(landed)) = (key, landed) else {
            return self.stand_nowhere(from, key);
        };
        if let Some(deck) = self.windows.deck_mut(from) {
            deck.stands();
        }
        self.windows.use_at(from, &self.shared);
        self.on(target).land(key, &landed);
    }
}
