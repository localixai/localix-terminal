//! Closing tabs: one or several at a call, each let go of by its owner and closed in the
//! ledger, as its kind closes.

use serde_json::Value;

use super::actions::{Kind, parse};
use super::args::{strings, text};
use super::{Call, Desk, Failure, Output};

impl Desk {
    pub(super) fn close(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        let mut keys = strings(args, "tabs");
        keys.extend(text(args, "tab").map(String::from));
        if keys.is_empty() {
            return Err("`tab` or `tabs` is needed: ids from `open` or `list`"
                .to_string()
                .into());
        }
        let said: Vec<String> = keys
            .iter()
            .map(|key| {
                let done = self
                    .tab(call, Some(key))
                    .and_then(|tab| self.close_tab(&tab.key));
                done.unwrap_or_else(|why| format!("{key}: {why}"))
            })
            .collect();
        Ok(said.join("\n").into())
    }

    /// Close tab `key` and forget it. A page is let go once the window has closed it (one the
    /// window lost is let go by `prune_pages`); a terminal or a board first, since its row of the
    /// ledger keeps the screen of a session that closing then takes away.
    pub(super) fn close_tab(&self, key: &str) -> Result<String, String> {
        let tab = parse(key, &self.made.kinds).ok_or_else(|| format!("`{key}` is not a tab id"))?;
        match tab.kind {
            Kind::Web => {
                let closed = self.close_web(tab.id)?;
                self.let_go(&tab.key);
                Ok(closed)
            }
            Kind::Term => {
                self.let_go(&tab.key);
                self.close_term(tab.id)
            }
            Kind::Board => {
                self.let_go(&tab.key);
                self.close_board(&tab.key)
            }
            Kind::File => {
                self.let_go(&tab.key);
                self.close_file(&tab.key)
            }
            Kind::Made(_) => {
                self.let_go(&tab.key);
                self.close_made(&tab.key)
            }
        }
    }

    /// Tab `key` is no one's any more, and closed in the ledger. Its owner hears `tab/closed`,
    /// unless the person opened it.
    pub(super) fn let_go(&self, key: &str) {
        let (owned, owner) = {
            let mut registry = crate::lock(&self.registry);
            let owner = registry.owner_named(key);
            let told = owner.clone().filter(|_| !registry.persons(key));
            registry.release(key);
            (owner.is_some(), told)
        };
        if owned {
            self.gone(owner, key);
        }
    }
}
