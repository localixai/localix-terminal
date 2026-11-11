//! The window's dialog (dialog.rs) on the event loop: a question is asked in it while the window
//! is on screen, and as the system asks otherwise; an update that is ready is announced in it
//! once; and the person's answer is carried to whoever waits.

use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use super::super::Cmd;
use super::On;
use crate::browser::{Confirm, Heard, dialog, platform, updates};

impl On<'_> {
    /// `question` asked of the person; `reply` takes whether they went on.
    pub(super) fn confirm(&mut self, question: &Confirm, reply: SyncSender<bool>) {
        let done = move |yes| {
            let _ = reply.send(yes);
        };
        match self.deck.as_ref() {
            Some(open) if open.dialogs() => {
                dialog::ask(dialog::Dialog::question(question), done);
                open.refresh(self.shared);
            }
            open => platform::confirm(question, open.map(|open| &open.frame.native), done),
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
        dialog::ask(announced, move |yes| {
            shared.offers.announced(&id, &offer, yes);
            if yes {
                shared.heard("", Heard::Update);
            }
        });
        open.refresh(self.shared);
    }

    /// The person answered the dialog shown: whoever waited is told, every window draws what is
    /// up now, and the keys go back to the tab shown.
    pub(super) fn dialog_answered(&mut self, yes: bool) {
        dialog::answer(yes);
        let _ = self.proxy.send_event(Cmd::Strip);
        if let Some(open) = self.deck.as_ref() {
            open.refresh(self.shared);
            open.focus_shown();
        }
    }
}
