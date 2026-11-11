//! The window's dialog: the one look of everything the app asks or announces in its window. A
//! mark that draws itself, a title, a sentence, what is new when there is news, and two buttons.
//! The strip's page draws it over the whole window (dialog.js, dialog.css); the dialogs up wait
//! here in the order they were asked, one shown at a time, each with whoever waits for its answer.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::Serialize;

/// The most lines of news a dialog lists: what a person reads before deciding, not a history.
const NOTES_MOST: usize = 40;

/// A line of what is new: a group's heading, a point under it, or a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Note {
    Head(String),
    Item(String),
    Text(String),
}

/// A dialog as the page draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Dialog {
    /// What it is about: asked again while it waits, it is not shown twice.
    #[serde(skip)]
    pub(super) id: String,
    /// The picture in its mark (dialog.js `DLG_MARKS`).
    pub(super) mark: &'static str,
    /// What the mark is about: `accent`, `danger` (it cannot be undone) or `neutral`.
    pub(super) tone: &'static str,
    /// The version that runs and the one on offer, rolled from one to the other under the mark.
    pub(super) versions: Option<(String, String)>,
    pub(super) title: String,
    pub(super) detail: String,
    pub(super) notes: Vec<Note>,
    /// The button that does it, the default one.
    pub(super) yes: String,
    /// The button that does not, and Escape.
    pub(super) no: String,
    /// The person asked for it just now, so it takes the keys. One the app raises on its own
    /// leaves them where they are: a Return typed into a terminal never answers it.
    pub(super) asked: bool,
}

impl Dialog {
    /// A question to the person as a dialog: go on, or Cancel.
    pub(super) fn question(question: &super::Confirm) -> Self {
        Self {
            id: format!("ask:{}", question.title),
            mark: "ask",
            tone: "neutral",
            versions: None,
            title: question.title.clone(),
            detail: question.detail.clone(),
            notes: vec![],
            yes: question.yes.clone(),
            no: "Cancel".into(),
            asked: true,
        }
    }
}

/// Markdown's marks taken out of a line: emphasis and code are read as plain words, a link as
/// its words. The page sets it as text.
fn plain(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find('[') {
        let (before, link) = rest.split_at(at);
        out.push_str(before);
        let named = link.find("](").and_then(|mid| {
            let end = link.get(mid..)?.find(')')?;
            Some((link.get(1..mid)?, link.get(mid + end + 1..)?))
        });
        match named {
            Some((words, after)) => {
                out.push_str(words);
                rest = after;
            }
            None => {
                out.push('[');
                rest = link.get(1..).unwrap_or_default();
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").replace('`', "")
}

/// A release's notes (Markdown: headings, points, sentences) as the lines a dialog lists. A
/// line that goes on from the one before joins it.
pub(super) fn notes(markdown: &str) -> Vec<Note> {
    let mut notes: Vec<Note> = Vec::new();
    let mut open = false;
    for line in markdown.lines() {
        let line = line.trim();
        if line.is_empty() {
            open = false;
            continue;
        }
        if let Some(head) = line.strip_prefix('#') {
            notes.push(Note::Head(plain(head.trim_start_matches('#').trim())));
            open = false;
        } else if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            notes.push(Note::Item(plain(item.trim())));
            open = true;
        } else if let (true, Some(Note::Item(last) | Note::Text(last))) = (open, notes.last_mut()) {
            last.push(' ');
            last.push_str(&plain(line));
        } else {
            notes.push(Note::Text(plain(line)));
            open = true;
        }
    }
    notes.truncate(NOTES_MOST);
    notes
}

/// Whoever waits for a dialog's answer.
type Waiting = Vec<Box<dyn FnOnce(bool)>>;

/// The dialogs up, the first the one shown.
#[derive(Default)]
struct Stage {
    up: VecDeque<(Dialog, Waiting)>,
}

impl Stage {
    /// `done` waits for `dialog`'s answer: behind the others up, or with the one already up
    /// that is about the same thing.
    fn ask(&mut self, dialog: Dialog, done: Box<dyn FnOnce(bool)>) {
        match self.up.iter_mut().find(|(up, _)| up.id == dialog.id) {
            Some((_, waiting)) => waiting.push(done),
            None => self.up.push_back((dialog, vec![done])),
        }
    }

    /// The dialog shown answered: whoever waited for it, to be told.
    fn answered(&mut self) -> Waiting {
        self.up
            .pop_front()
            .map(|(_, waiting)| waiting)
            .unwrap_or_default()
    }
}

thread_local! {
    /// The dialogs up in the app: asked and answered on the main thread alone.
    static STAGE: Mutex<Stage> = Mutex::default();
}

/// `dialog` shown once those before it are answered; `done` takes whether the person chose its
/// `yes`.
pub(super) fn ask(dialog: Dialog, done: impl FnOnce(bool) + 'static) {
    STAGE.with(|stage| crate::lock(stage).ask(dialog, Box::new(done)));
}

/// The dialog shown now.
pub(super) fn shown() -> Option<Dialog> {
    STAGE.with(|stage| {
        crate::lock(stage)
            .up
            .front()
            .map(|(dialog, _)| dialog.clone())
    })
}

/// Whether a dialog is up.
pub(super) fn up() -> bool {
    STAGE.with(|stage| !crate::lock(stage).up.is_empty())
}

/// Whether the dialog about `id` is up, shown or behind another.
pub(super) fn waits(id: &str) -> bool {
    STAGE.with(|stage| {
        crate::lock(stage)
            .up
            .iter()
            .any(|(dialog, _)| dialog.id == id)
    })
}

/// The person answered the dialog shown: whoever waited is told, the stage let go of first (one
/// told may ask again).
pub(super) fn answer(yes: bool) {
    let waiting = STAGE.with(|stage| crate::lock(stage).answered());
    for done in waiting {
        done(yes);
    }
}

/// The page's word of the person's answer (`dialog yes`, `dialog no`), if `said` is one.
pub(super) fn said(said: &str) -> Option<bool> {
    match said {
        "dialog yes" => Some(true),
        "dialog no" => Some(false),
        _ => None,
    }
}

/// The dialog's call in the strip's page: `DLG(dialog)`, or `DLG(null)` with none up.
pub(super) fn script(dialog: Option<&Dialog>) -> String {
    format!(
        "DLG({})",
        serde_json::to_string(&dialog).unwrap_or_else(|_| "null".into())
    )
}

#[cfg(test)]
#[path = "dialog_tests.rs"]
mod tests;
