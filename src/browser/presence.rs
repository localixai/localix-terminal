//! The window's three states on macOS as pure logic: events in, what to do out. Hidden while no
//! tab is open; the window, an ordinary one, whenever one is; away, when the person puts it out
//! of sight (the yellow button, ⌘M): nothing is on screen, the tabs stay and keep running, and
//! only they bring the window back (the app's icon, `llxterm open`). Nothing the agent does
//! changes the state: its acts only turn the spinner of whose turn it is. Where the window cannot
//! be put away ([`Support::WindowOnly`]) it stays the window. The AppKit side
//! (host_presence.rs) applies what this decides.

/// Where the window is, as the person sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Hidden,
    /// The person put the window away: tabs are open, and nothing is on screen.
    Away,
    Expanded,
}

/// Whether this system lets the window be put away and still draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Support {
    Away,
    WindowOnly,
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Event {
    /// A window with tabs is on screen, where there was none.
    Appeared,
    /// The window is gone (no window at all, not one with no tab: that stays).
    Vanished,
    /// The person brought the window back: the app's icon, `llxterm open`.
    Expand,
    /// The person put the window away: the yellow button, ⌘M.
    Collapse,
    /// The agent began an action.
    Acting,
    /// The agent's action settled.
    Settled,
}

/// What to do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Effect {
    /// Put the window as this state says.
    Enter(State),
    /// Whose turn it is: the agent's while it acts.
    Busy(bool),
}

/// Whether the agent is in the middle of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Agent {
    Idle,
    Acting,
}

pub(super) struct Presence {
    state: State,
    support: Support,
    agent: Agent,
}

impl Presence {
    pub(super) fn new(support: Support) -> Self {
        Self {
            state: State::Hidden,
            support,
            agent: Agent::Idle,
        }
    }

    pub(super) fn state(&self) -> State {
        self.state
    }

    /// What `event` makes happen.
    pub(super) fn event(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::Appeared => self.appear(),
            Event::Vanished => self.vanish(),
            Event::Expand => self.enter(State::Away, State::Expanded),
            Event::Collapse if self.support == Support::Away => {
                self.enter(State::Expanded, State::Away)
            }
            Event::Collapse => vec![],
            Event::Acting => self.agent(Agent::Acting),
            Event::Settled => self.agent(Agent::Idle),
        }
    }

    /// Tabs came: the window, which the person may put away later.
    fn appear(&mut self) -> Vec<Effect> {
        self.enter(State::Hidden, State::Expanded)
    }

    fn vanish(&mut self) -> Vec<Effect> {
        if self.state == State::Hidden {
            return vec![];
        }
        self.state = State::Hidden;
        self.agent = Agent::Idle;
        vec![Effect::Enter(State::Hidden)]
    }

    /// `to` when the window is `from`, else nothing.
    fn enter(&mut self, from: State, to: State) -> Vec<Effect> {
        if self.state != from {
            return vec![];
        }
        self.state = to;
        vec![Effect::Enter(to)]
    }

    fn agent(&mut self, now: Agent) -> Vec<Effect> {
        if self.state == State::Hidden || self.agent == now {
            return vec![];
        }
        self.agent = now;
        vec![Effect::Busy(now == Agent::Acting)]
    }
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod tests;
