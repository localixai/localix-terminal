//! What the browser tells the app, as plain values with nothing of the engine in them: a tab as
//! the agent reads it, and what the person did in the window.

/// Tabs open at once: each is a WebView with its own web process.
pub const MAX_TABS: usize = 8;

/// What the person did in the window, in words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard<'a> {
    /// They did something in a tab (went to an address): news for whoever works in it.
    Did(&'a str),
    /// They closed a tab: news for whoever works in it.
    Closed(&'a str),
    /// They closed the window.
    WindowClosed(&'a str),
    /// They sent their comments on a page: words for the agent, as a message of theirs.
    Sent(&'a str),
    /// They asked for a terminal of their own ("+"), in the project the key names (a group:
    /// the project of the tab shown, empty when it has none).
    Terminal,
}

/// One tab as the agent reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub id: u32,
    pub url: String,
    pub title: String,
}
