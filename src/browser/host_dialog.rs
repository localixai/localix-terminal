//! The window's dialog (dialog.rs) on the event loop: a question is asked in it while the window
//! is on screen, and as the system asks otherwise; an update that is ready is announced in it
//! once; and the person's answer is carried to whoever waits.

use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use super::super::Cmd;
use super::On;
use crate::browser::{Confirm, Heard, dialog, platform, updates};
use crate::kind::{Given, Question};

impl On<'_> {
    /// `question` asked of the person; `reply` takes whether they went on.
    pub(super) fn confirm(&mut self, question: &Confirm, reply: SyncSender<bool>) {
        let done = move |yes, _: &Given| {
            let _ = reply.send(yes);
        };
        match self.deck.as_ref() {
            Some(open) if open.dialogs() => {
                dialog::ask(dialog::Dialog::question(question), done);
                open.refresh(self.shared);
            }
            open => {
                let done = move |yes| done(yes, &Given::new());
                platform::confirm(question, open.map(|open| &open.frame.native), done);
            }
        }
    }

    /// A tab's `question` asked of the person; `reply` takes what they gave when they went on.
    /// With the window away it is asked as the system asks, which has no fields: what the
    /// question needs is then missing, and whoever asked says so.
    pub(super) fn question(&mut self, question: &Question, reply: SyncSender<Option<Given>>) {
        let done = move |yes: bool, given: &Given| {
            let _ = reply.send(yes.then(|| given.clone()));
        };
        match self.deck.as_ref() {
            Some(open) if open.dialogs() => {
                dialog::ask(dialog::Dialog::asking(question), done);
                open.refresh(self.shared);
            }
            open => {
                let plain = Confirm {
                    title: question.title.clone(),
                    detail: [question.detail.as_str()]
                        .into_iter()
                        .chain(question.points.iter().map(String::as_str))
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    yes: question.yes.clone(),
                };
                let done = move |yes| done(yes, &Given::new());
                platform::confirm(&plain, open.map(|open| &open.frame.native), done);
            }
        }
    }

    /// An update that is ready is announced, once a version: Later puts its offer off as Not Now
    /// does, and the update stays in Settings.
    pub(super) fn announce(&mut self) {
        let Some(open) = self.deck.as_ref().filter(|open| open.dialogs()) else {
            return;
        };
        let told = crate::update::app().told();
        let Some((offer, announced)) = updates::announcement(&told) else {
            return;
        };
        if dialog::waits(&announced.id) || !self.shared.offers.due(&announced.id) {
            return;
        }
        let (shared, id) = (Arc::clone(self.shared), announced.id.clone());
        dialog::ask(announced, move |yes, _| {
            shared.offers.announced(&id, &offer, yes);
            if yes {
                shared.heard("", Heard::Update);
            }
        });
        open.refresh(self.shared);
    }

    /// The person answered the dialog shown: whoever waited is told, every window draws what is
    /// up now, and the keys go back to the tab shown.
    pub(super) fn dialog_answered(&mut self, yes: bool, given: &Given) {
        dialog::answer(yes, given);
        let _ = self.proxy.send_event(Cmd::Strip);
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
            open.focus_shown();
        }
    }
}
