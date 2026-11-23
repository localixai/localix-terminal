//! A link the person Command-clicks in a terminal: where it opens, by Settings › Terminal › Open
//! links, or, while that says Ask, by their answer to the window's dialog: in a tab of the
//! window, or in their default browser. Only a web address (http, https) is followed: a
//! program's output is untrusted, and a link to a file or to another app's scheme would hand
//! it to whatever the system runs for it.

use super::Confirm;
use super::dialog::Answer;
use super::settings::OpenLinks;

/// Where a link opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Place {
    /// A page tab of the window, in the terminal's project.
    Tab,
    /// The person's default browser, as the system opens an address.
    Browser,
}

/// What a Command-click on a link does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Clicked {
    /// The person is asked where it opens (`question`).
    Ask,
    /// It opens there at once.
    Open(Place),
}

/// What a Command-click on `address` does with Open links set to `setting`; nothing for an
/// address that is no web page's.
pub(super) fn clicked(setting: OpenLinks, address: &str) -> Option<Clicked> {
    super::panel::web(address).then_some(match setting {
        OpenLinks::Ask => Clicked::Ask,
        OpenLinks::Tab => Clicked::Open(Place::Tab),
        OpenLinks::Browser => Clicked::Open(Place::Browser),
    })
}

/// The question a click asks: the address is its sentence, a tab its act (the default button,
/// Return), the browser its second act, and Cancel opens nothing.
pub(super) fn question(address: &str) -> Confirm {
    Confirm {
        title: "Open Link?".into(),
        detail: address.into(),
        yes: "Open in Tab".into(),
        other: Some("Open in Browser".into()),
    }
}

/// Where the person's answer to `question` opens the link; none when they cancelled.
pub(super) fn answered(answer: Answer) -> Option<Place> {
    match answer {
        Answer::Yes => Some(Place::Tab),
        Answer::Other => Some(Place::Browser),
        Answer::No => None,
    }
}

#[cfg(test)]
#[path = "term_links_tests.rs"]
mod tests;
