//! A tab's side of what its pages tell (notices.rs): where a call starts reading them, what came
//! since, and how the tab answers a confirm or a prompt while a call armed it.

use serde_json::json;

use crate::browser::{Armed, Notice};
use crate::view::ViewResult;

use super::WebviewDriver;

impl WebviewDriver {
    /// Where the notices are now: a call reads what its tab was told after this.
    pub fn told_mark(&self) -> u64 {
        self.shared.notices.mark()
    }

    /// What this tab's pages told after `mark`.
    pub fn told_since(&self, mark: u64) -> Vec<Notice> {
        self.shared.notices.since(self.target(), mark)
    }

    /// From now on this tab answers a confirm or a prompt OK (`armed`), or Cancel (none): the
    /// document shown is told at once, a later one before each action.
    pub fn arm(&self, armed: Option<Armed>) {
        let tab = self.target();
        let was = self.shared.notices.armed(tab).is_some();
        let arming = armed.is_some();
        self.shared.notices.arm(tab, armed);
        if arming || was {
            let _ = self.tell_answer();
        }
    }

    /// Before an action: a document that came since the tab was armed starts at Cancel.
    pub(super) fn before_acting(&self) {
        if self.shared.notices.armed(self.target()).is_some() {
            let _ = self.tell_answer();
        }
    }

    fn tell_answer(&self) -> ViewResult<()> {
        let args = match self.shared.notices.armed(self.target()) {
            Some(armed) => json!({"accept": true, "text": armed.text}),
            None => json!({"accept": false}),
        };
        // Waits for a probe between two documents: with no answer the new document starts at
        // Cancel, though the call asked for OK. Only an armed tab (or one just disarmed) gets here.
        self.call_ready("answer", &args).map(|_| ())
    }
}
