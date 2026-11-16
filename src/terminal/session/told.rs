//! What a session tells whoever listens besides its screen: that its program has another note for
//! the person, and that its shell ended a command (with whether the person's own keys typed it);
//! and the command of theirs that ended while they were not looking, kept until they type in the
//! terminal.

use std::sync::Arc;

use super::super::Ended;
use super::{Session, Shared};

/// Hears each command the shell ended, and whether the person typed it.
type Commanded = dyn Fn(Ended, bool) + Send + Sync;

#[derive(Default)]
pub(super) struct Told {
    /// Called when what waits for the person in the terminal comes or goes (the program's note,
    /// a command of theirs that ended unseen), and the number of the last note it was called for.
    pub(super) on_note: Option<Arc<dyn Fn() + Send + Sync>>,
    pub(super) noted: u32,
    on_command: Option<Arc<Commanded>>,
    /// How many commands the shell had ended when `on_command` was last called.
    commands: u64,
    /// The last command of the person's that ended unseen, by its number; a newer one replaces it.
    pub(super) unseen: Option<(u32, Ended)>,
    numbered: u32,
}

impl Session {
    /// `heard` with each command the shell says it ended from here on (once, however often its
    /// end is said), and whether the person's own keys typed it: an agent's `type_in` does not.
    pub fn on_command(&self, heard: impl Fn(Ended, bool) + Send + Sync + 'static) {
        let mut state = self.shared.lock();
        state.told.commands = state.screen.marks().ended().0;
        state.told.on_command = Some(Arc::new(heard));
    }

    /// A command of the person's ended while they were not looking (the window decides): it waits
    /// for them, numbered, until they type in the terminal.
    pub fn unseen(&self, ended: Ended) {
        let mut state = self.shared.lock();
        let told = &mut state.told;
        told.numbered = told.numbered.wrapping_add(1);
        told.unseen = Some((told.numbered, ended));
    }

    /// The command of the person's that ended unseen and waits for them, with its number.
    pub fn unseen_command(&self) -> Option<(u32, Ended)> {
        self.shared.lock().told.unseen
    }
}

/// Whoever listens told that the program has another note for the person.
pub(super) fn noted(shared: &Shared) {
    let heard = {
        let mut state = shared.lock();
        let number = state.screen.notes().waiting().map(|(number, _)| number);
        match number {
            Some(number) if number != state.told.noted => state.told.noted = number,
            _ => return,
        }
        state.told.on_note.clone()
    };
    if let Some(heard) = heard {
        heard();
    }
}

/// Whoever listens told that the shell ended a command since it was last told: the last of them,
/// once.
pub(super) fn commanded(shared: &Shared) {
    let heard = {
        let mut state = shared.lock();
        let (count, ended) = state.screen.marks().ended();
        if count == state.told.commands {
            return;
        }
        state.told.commands = count;
        let theirs = state.screen.marks().theirs();
        (state.told.on_command.clone()).zip(ended.map(|ended| (ended, theirs)))
    };
    if let Some((heard, (ended, theirs))) = heard {
        heard(ended, theirs);
    }
}

#[cfg(test)]
#[path = "told_tests.rs"]
mod tests;
