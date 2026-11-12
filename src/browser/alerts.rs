//! What waits for the person, said outside the Event Center's list: which tabs wait and with what
//! words (the strip marks them), and, while the person is in another app, each new thing as a
//! notice of the system's, taken back once it no longer waits; the Dock counts what is new. All of
//! it is read from the center's one list of events, so the three never disagree with it.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

/// One event of the center, as much as is said outside its list.
#[derive(Debug, Deserialize)]
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "only macOS's notices read the whole of an event")
)]
pub(super) struct Event {
    n: u64,
    section: String,
    urgent: bool,
    unseen: bool,
    #[serde(default)]
    group: String,
    #[serde(default)]
    tab: String,
    title: String,
    #[serde(default)]
    detail: String,
    /// What its tab waits with, where its title only names the tab; none: the title.
    #[serde(default)]
    words: String,
    #[serde(default)]
    buttons: Vec<Button>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "only macOS's notices ask what a button does")
)]
struct Button {
    role: String,
}

/// The center's `events` (its JSON list, newest first); none when there is no list to read.
pub(super) fn read(events: &str) -> Vec<Event> {
    serde_json::from_str(events).unwrap_or_default()
}

/// The tabs that wait for the person, each with the words of what wants them there most: the
/// first of its events in the list's order.
pub(super) fn waits(events: &[Event]) -> HashMap<String, String> {
    let mut waits = HashMap::new();
    for event in events
        .iter()
        .filter(|event| event.urgent && !event.tab.is_empty())
    {
        waits
            .entry(event.tab.clone())
            .or_insert_with(|| event.says().0.to_string());
    }
    waits
}

/// A notice of the system's for one event.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "only macOS has the system's notices")
)]
pub(super) struct Note {
    /// The event's number: the notice is taken back by it.
    pub id: u64,
    pub title: String,
    pub body: String,
    /// What a click on it says, as a click in the strip does (`event 7 0`, `events`).
    pub click: String,
}

/// What changes outside the list after the events were heard.
#[derive(Debug, Default)]
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(
        dead_code,
        reason = "only macOS has the system's notices and a Dock to count on"
    )
)]
pub(super) struct Change {
    pub post: Vec<Note>,
    pub withdraw: Vec<u64>,
    /// How many new things want the person: the Dock's count.
    pub badge: usize,
}

/// What was told so far, so each thing is told once.
#[derive(Debug, Default)]
#[cfg_attr(
    all(not(target_os = "macos"), not(test)),
    expect(dead_code, reason = "only macOS has the system's notices")
)]
pub(super) struct Alerts {
    /// What wanted the person when last heard: only what is not among it is news.
    known: HashSet<u64>,
    /// The notices up.
    posted: HashSet<u64>,
    /// The list was heard before. What it holds when first heard waited before the app was
    /// started: counted, and not news.
    begun: bool,
}

impl Alerts {
    /// The events as they are now, the person in the app (`active`) or away.
    #[cfg_attr(
        all(not(target_os = "macos"), not(test)),
        expect(dead_code, reason = "only macOS has the system's notices")
    )]
    pub(super) fn heard(&mut self, events: &[Event], active: bool) -> Change {
        let wanting: Vec<_> = (events.iter())
            .filter(|event| event.urgent && event.unseen)
            .collect();
        let now: HashSet<u64> = wanting.iter().map(|event| event.n).collect();
        let post: Vec<Note> = (wanting.iter().rev())
            .filter(|event| self.begun && !active && !self.known.contains(&event.n))
            .map(|event| event.note())
            .collect();
        let withdraw: Vec<u64> = self.posted.difference(&now).copied().collect();
        self.posted.retain(|id| now.contains(id));
        self.posted.extend(post.iter().map(|note| note.id));
        self.known = now;
        self.begun = true;
        Change {
            post,
            withdraw,
            badge: wanting.len(),
        }
    }
}

/// The center's `events` as the host has them now, said outside the list: once for each change
/// of them, on the loop's thread.
#[cfg(target_os = "macos")]
pub(super) fn told(events: &str, proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {
    static TOLD: std::sync::Mutex<Option<(String, Alerts)>> = std::sync::Mutex::new(None);
    // Only the app's bundle is an app the system gives notices and a Dock count to; a window
    // the person does not see (a test's) says nothing either.
    if crate::bundle::app().is_none() || super::platform::unseen() {
        return;
    }
    let mut told = crate::lock(&TOLD);
    let (last, alerts) = told.get_or_insert_with(Default::default);
    if last == events {
        return;
    }
    events.clone_into(last);
    let change = alerts.heard(&read(events), super::alerts_macos::active());
    super::alerts_macos::carry(&change, proxy);
}

/// Elsewhere the strip's marks say it all.
#[cfg(not(target_os = "macos"))]
pub(super) fn told(_events: &str, _proxy: &tao::event_loop::EventLoopProxy<super::host::Cmd>) {}

impl Event {
    /// What it says, and what stands beside that: its words on its tab and the program they
    /// are of, or its title and its detail.
    fn says(&self) -> (&str, &str) {
        if self.words.is_empty() {
            (&self.title, &self.detail)
        } else {
            (&self.words, &self.title)
        }
    }

    /// The notice of the system's for it.
    #[cfg_attr(
        all(not(target_os = "macos"), not(test)),
        expect(dead_code, reason = "only macOS has the system's notices")
    )]
    fn note(&self) -> Note {
        let open = self.buttons.iter().position(|button| button.role == "open");
        let (title, beside) = self.says();
        let body = [&*self.section, beside, &*self.group];
        Note {
            id: self.n,
            title: title.to_string(),
            body: (body.iter().filter(|part| !part.is_empty()).copied())
                .collect::<Vec<_>>()
                .join(" · "),
            // The list is opened, never shut, where there is no tab to show.
            click: open.map_or_else(
                || "events open".into(),
                |at| format!("event {} {at}", self.n),
            ),
        }
    }
}

#[cfg(test)]
#[path = "alerts_tests.rs"]
mod tests;
