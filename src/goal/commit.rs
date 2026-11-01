//! Controls whose click commits something: paying or ordering (a purchase, a booking), deleting,
//! and sending (a form, a message, a save). Known by the words of their names, whole. A goal may
//! pay or delete only when its own words say so (the caller's words: a page cannot add them), a
//! run never clicks the same paying control twice once it acted, and the report names every step
//! that may have committed something, so the caller reads the page before doing it again.

use crate::goal::step::{GoalStep, Operation};
use crate::view::Element;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commit {
    Pays,
    Deletes,
    Sends,
}

/// A control's words that pay, delete or send; a pair counts as one.
const PAYS: [&str; 9] = [
    "pay",
    "buy",
    "purchase",
    "checkout",
    "subscribe",
    "donate",
    "reserve",
    "book",
    "rent",
];
const PAYS_PAIRS: [&str; 6] = [
    "place order",
    "confirm order",
    "complete order",
    "submit order",
    "order now",
    "check out",
];
const DELETES: [&str; 8] = [
    "delete",
    "remove",
    "erase",
    "destroy",
    "discard",
    "unsubscribe",
    "deactivate",
    "trash",
];
// Not "apply": it applies a filter or a promo code far more often than it sends an application.
const SENDS: [&str; 8] = [
    "send", "submit", "post", "publish", "register", "confirm", "save", "signup",
];
const SENDS_PAIRS: [&str; 2] = ["sign up", "create account"];

/// Words in a control's name read whole; a longer name is read by its first words.
const SHORT: usize = 4;

/// How a goal says it may pay, or delete: the start of one of its words.
const ASKS_TO_PAY: [&str; 11] = [
    "pay", "buy", "bought", "purchas", "checkout", "order", "subscri", "donat", "reserv", "book",
    "rent",
];
const ASKS_TO_DELETE: [&str; 8] = [
    "delet",
    "remov",
    "eras",
    "destroy",
    "discard",
    "unsubscri",
    "deactivat",
    "trash",
];

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(String::from)
        .collect()
}

impl Commit {
    /// What clicking `element` commits, by its name: none for most controls.
    pub fn of(element: &Element) -> Option<Self> {
        if !matches!(
            element.role.as_str(),
            "button" | "link" | "menuitem" | "gridcell" | "option"
        ) {
            return None;
        }
        // Its words, not its numbers ("Pay $54.20"). A short name is read whole ("Confirm and
        // pay"); a long one by its first words, its verb, since the rest names what it acts on
        // ("Edit Buy oat milk" edits).
        let words: Vec<String> = (words(&element.name).into_iter())
            .filter(|word| word.chars().any(char::is_alphabetic))
            .collect();
        let joined = words.join(" ");
        let short = words.len() <= SHORT;
        let first = words.first().map(String::as_str).unwrap_or_default();
        let any = |list: &[&str]| {
            list.contains(&first) || (short && words.iter().any(|w| list.contains(&w.as_str())))
        };
        let pair = |list: &[&str]| {
            (list.iter()).any(|pair| joined.starts_with(pair) || (short && joined.contains(pair)))
        };
        if any(&PAYS) || pair(&PAYS_PAIRS) {
            Some(Self::Pays)
        } else if any(&DELETES) {
            Some(Self::Deletes)
        } else if any(&SENDS) || pair(&SENDS_PAIRS) {
            Some(Self::Sends)
        } else {
            None
        }
    }

    /// What a goal must say for this to be clicked: nothing for sending. Only what it asks to
    /// do counts, not how to tell it is done ("done when the order shows" asks for no order).
    fn asked(self, goal: &str) -> bool {
        let stems: &[&str] = match self {
            Self::Pays => &ASKS_TO_PAY,
            Self::Deletes => &ASKS_TO_DELETE,
            Self::Sends => return true,
        };
        let words = asked_words(goal);
        // Two words that ask to pay together, where neither does alone.
        let checks_out = words.windows(2).any(|pair| pair == ["check", "out"]);
        (self == Self::Pays && checks_out)
            || (words.iter()).any(|word| stems.iter().any(|stem| word.starts_with(stem)))
    }

    /// What the report says the step may have done.
    pub fn done(self) -> &'static str {
        match self {
            Self::Pays => "may have paid or ordered",
            Self::Deletes => "may have deleted",
            Self::Sends => "may have sent or saved",
        }
    }
}

/// Where a goal stops saying what to do and starts saying how to tell it is done.
const DONE_WHEN: [&str; 6] = [
    "done when",
    "done once",
    "done if",
    "until",
    "stop when",
    "finished when",
];
/// The start of a word of a goal that asks to do again what was done before.
const ASKS_AGAIN: [&str; 5] = ["again", "another", "twice", "repeat", "second"];

/// What `goal` asks to do: its words before how to tell it is done.
fn asks(goal: &str) -> &str {
    let lower = goal.to_lowercase();
    let end = (DONE_WHEN.iter())
        .filter_map(|marker| lower.find(marker))
        .min()
        .unwrap_or(lower.len());
    // Lowercasing keeps ASCII offsets; a goal whose case changes its length is read whole.
    goal.get(..end)
        .filter(|_| lower.len() == goal.len())
        .unwrap_or(goal)
}

/// Words that say not to do what follows them in their clause ("without booking", "do not
/// pay", "don't delete": the `t` of a "n't").
const NOT: [&str; 9] = [
    "not", "no", "never", "without", "nor", "don", "doesn", "cannot", "t",
];

/// The words of what `goal` asks to do that it does not say not to: in each clause (between
/// punctuation, or a "but"), the words before any of [`NOT`].
fn asked_words(goal: &str) -> Vec<String> {
    let mut asked = vec![];
    for clause in asks(goal).split([',', ';', ':', '.', '!', '?']) {
        let mut said_not = false;
        for word in words(clause) {
            if word == "but" {
                said_not = false;
            } else if NOT.contains(&word.as_str()) {
                said_not = true;
            } else if !said_not {
                asked.push(word);
            }
        }
    }
    asked
}

/// Why `element` is held back from a click in this run, or none: paying or deleting the goal
/// does not ask for, or a paying control this run, or an earlier goal on this view
/// (`paid_before`), already clicked to some effect, unless the goal asks to do it again.
pub(crate) fn held_back(
    element: &Element,
    goal: &str,
    history: &[GoalStep],
    paid_before: &[String],
) -> Option<String> {
    let commit = Commit::of(element)?;
    if !commit.asked(goal) {
        let what = match commit {
            Commit::Pays => "pay or order",
            Commit::Deletes => "delete",
            Commit::Sends => return None,
        };
        return Some(format!(
            "{}: the goal does not ask to {what}",
            element.label()
        ));
    }
    // A click on it, or Enter in its form's field.
    let paid = history.iter().any(|step| {
        step.commit == Some(Commit::Pays)
            && step.failed.is_none()
            && step.changed == Some(true)
            && step.control.as_deref() == Some(element.label().as_str())
    });
    if commit != Commit::Pays {
        return None;
    }
    if paid {
        return Some(format!(
            "{}: this goal clicked it once already",
            element.label()
        ));
    }
    let again = asked_words(goal)
        .iter()
        .any(|word| ASKS_AGAIN.iter().any(|stem| word.starts_with(stem)));
    (paid_before.contains(&element.label()) && !again).then(|| {
        format!(
            "{}: an earlier goal in this tab clicked it already (to do it again, the goal says so)",
            element.label()
        )
    })
}

/// The paying, deleting or sending control of the form that Enter in the field last typed into
/// would send, or none: a field outside a form, or a form with no such control. Paying first,
/// then deleting, then sending.
pub(crate) fn sent_by_enter<'a>(
    elements: &'a [Element],
    history: &[GoalStep],
) -> Option<&'a Element> {
    let typed = (history.last())
        .filter(|step| step.operation == Operation::TypeText)?
        .reference
        .as_deref()?;
    let form = (elements.iter())
        .find(|element| element.reference == typed)?
        .form
        .as_deref()?;
    let rank = |commit: Commit| match commit {
        Commit::Pays => 3,
        Commit::Deletes => 2,
        Commit::Sends => 1,
    };
    (elements.iter())
        .filter(|element| element.form.as_deref() == Some(form))
        .filter_map(|element| Commit::of(element).map(|commit| (rank(commit), element)))
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, element)| element)
}

/// Whether `operation` (with `value`, a key's name) is Enter: what sends a form from its field.
pub(crate) fn is_enter(operation: Operation, value: Option<&str>) -> bool {
    operation == Operation::PressEnter
        || (operation == Operation::PressKey && value == Some("Enter"))
}

#[cfg(test)]
#[path = "commit_tests.rs"]
mod tests;
