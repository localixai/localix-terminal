//! Applies the window's presence (presence.rs) on the event loop: the deck concealed or
//! revealed, the keys given back to the app the person was in when they put it away, and where
//! they left it remembered. The agent's acts reach it as events, and nothing here brings the
//! window back but the person's click.

use std::sync::Arc;

use tao::event::WindowEvent;

use super::super::deck::Deck;
use super::super::placement::Rect;
use super::super::platform;
use super::super::presence::{Effect, Event, Presence, State, Support};
use super::Shared;

/// What an effect is applied with.
pub(super) struct Stage<'a> {
    pub deck: &'a mut Option<Deck>,
    pub shared: &'a Arc<Shared>,
}

pub(super) struct Presenter {
    presence: Presence,
}

impl Presenter {
    pub(super) fn new() -> Self {
        let support = if cfg!(target_os = "macos") {
            Support::Away
        } else {
            Support::WindowOnly
        };
        Self {
            presence: Presence::new(support),
        }
    }

    /// The state as a check reads it.
    pub(super) fn name(&self) -> &'static str {
        match self.presence.state() {
            State::Hidden => "hidden",
            State::Away => "away",
            State::Expanded => "expanded",
        }
    }

    pub(super) fn event(&mut self, event: Event, stage: &mut Stage<'_>) {
        for effect in self.presence.event(event) {
            Self::apply(effect, stage);
        }
    }

    /// The person opened the app (its Dock icon, `llx-desk open`): a window put away comes back,
    /// and llx-desk becomes the app they use, its window taking their next click and key. Only
    /// their click does this: what the agent shows never takes their keys.
    pub(super) fn opened(&mut self, stage: &mut Stage<'_>) {
        self.event(Event::Expand, stage);
        if self.presence.state() == State::Expanded
            && let Some(deck) = stage.deck.as_ref()
        {
            deck.frame.focus();
        }
    }

    /// After every turn of the loop: a window that came or went.
    pub(super) fn sync(&mut self, stage: &mut Stage<'_>) {
        let shown = stage.deck.as_ref().is_some_and(Deck::is_shown);
        match (shown, self.presence.state()) {
            (true, State::Hidden) => self.event(Event::Appeared, stage),
            (false, State::Away | State::Expanded) => self.event(Event::Vanished, stage),
            (true, State::Away | State::Expanded) | (false, State::Hidden) => {}
        }
    }

    fn apply(effect: Effect, stage: &mut Stage<'_>) {
        match effect {
            Effect::Enter(state) => Self::enter(state, stage),
            Effect::Busy(on) => {
                stage.shared.turn.acting(on);
                if let Some(deck) = stage.deck.as_ref() {
                    deck.refresh(stage.shared);
                }
            }
        }
    }

    fn enter(state: State, stage: &mut Stage<'_>) {
        let Some(deck) = stage.deck.as_mut() else {
            return;
        };
        // Out of sight, the person points at nothing: commenting ends, and the agent's actions
        // that waited for them go. Comments not sent yet are kept for Send.
        if state != State::Expanded {
            deck.stop_commenting(stage.shared);
        }
        match state {
            State::Hidden => {}
            // Out of sight and still drawing, so what the agent does goes on; the keys go back
            // to the app the person was in.
            State::Away => {
                deck.frame.conceal();
                platform::give_up_keys();
            }
            State::Expanded => deck.frame.reveal(),
        }
    }

    /// The person moved or sized the expanded window: that is its frame next time.
    pub(super) fn deck_moved(&self, deck: &Deck, event: &WindowEvent<'_>, shared: &Shared) {
        let moved = matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_));
        if !moved || self.presence.state() != State::Expanded {
            return;
        }
        let native = &deck.frame.native;
        // Full screen and zoomed are not a size the person chose: a window opened meanwhile
        // would be as large as the screen.
        if native.fullscreen().is_some() || native.is_maximized() {
            return;
        }
        let scale = native.scale_factor();
        if let Ok(at) = native.outer_position() {
            let (at, size) = (
                at.to_logical::<f64>(scale),
                native.outer_size().to_logical::<f64>(scale),
            );
            crate::lock(&shared.kept).deck = Some(Rect {
                x: at.x,
                y: at.y,
                width: size.width,
                height: size.height,
            });
        }
    }
}
