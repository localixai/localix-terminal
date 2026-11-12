//! What the Store asks the person before it is done, each in the window's dialog (`Tab::ask`),
//! the one every question of the app is asked in: an install, with what it does and what it
//! needs typed; a source removed; a source added. The page draws none of it. A question waits
//! for the person, so each is asked off the thread that brought it, and one already up is not
//! asked twice.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};

use super::Store;
use crate::catalog::Listing;
use crate::catalog::target::Target;
use crate::kind::{Field, Given, Question, Tab};

impl Store {
    /// The person is asked to install listing `id` in agent `agent`, and with a yes it is
    /// done. `from_agent`, an agent asked for it: it reads that the answer is waited for, and
    /// then what it was. What is said back is what the install would do.
    pub(super) fn ask_install(
        &self,
        id: &str,
        agent: &str,
        from_agent: bool,
        tab: &Arc<dyn Tab>,
    ) -> Result<Vec<String>, String> {
        let listing = self.listing(id)?;
        let target = self.target(agent)?;
        let env = self.env();
        if !target.takes(&listing) {
            return Err(format!("{} does not take {}", target.name(), listing.title));
        }
        if !target.present(env) {
            return Err(format!("{} was not found on this Mac", target.name()));
        }
        let plan = target.plan(&listing, env)?;
        let lead = if from_agent {
            "An agent asks to install"
        } else {
            "Install"
        };
        let does = if listing.runs_code() {
            "It puts code on this Mac that the agent runs."
        } else {
            "It adds this to the agent."
        };
        let question = Question {
            title: format!("{lead} {} in {}?", listing.title, target.name()),
            detail: [does, plan.after.as_str()].join(" ").trim().to_string(),
            heading: "What It Does".into(),
            points: plan.steps.clone(),
            fields: listing.needs().iter().map(field).collect(),
            yes: "Install".into(),
            danger: false,
            asked: !from_agent,
        };
        let key = format!("install {id} {agent}");
        let waits = json!({"id": listing.id, "agent": target.id()});
        let tab = Arc::clone(tab);
        self.asking(
            key,
            &tab,
            question,
            move |store, given| {
                if from_agent {
                    crate::lock(&store.state).asked = None;
                }
                let Some(given) = given else {
                    if from_agent {
                        let (id, agent) = (&listing.id, target.id());
                        store.note(format!("The person declined to install {id} in {agent}."));
                    }
                    return Ok(());
                };
                store.install_with(listing, target, &given)
            },
            from_agent.then_some(waits),
        );
        Ok(plan.steps)
    }

    /// `listing` installed in `target` with what the person gave for what it needs: only what
    /// it asks for is taken, and what it requires must be there.
    fn install_with(
        &self,
        listing: Listing,
        target: &'static dyn Target,
        given: &Given,
    ) -> Result<(), String> {
        let mut values = BTreeMap::new();
        for need in listing.needs() {
            let value = (given.get(&need.name).map(String::as_str))
                .filter(|value| !value.is_empty())
                .or_else(|| Some(need.default.as_str()).filter(|default| !default.is_empty()));
            match value {
                Some(value) => {
                    values.insert(need.name.clone(), value.to_string());
                }
                None if need.required => return Err(format!("{} is needed", need.name)),
                None => {}
            }
        }
        self.change(listing, target, Some(values));
        Ok(())
    }

    /// The person is asked whether source `id` goes, and with a yes it does.
    pub(super) fn ask_remove_source(&self, id: &str, tab: &Arc<dyn Tab>) {
        let kept = {
            let state = crate::lock(&self.state);
            (state.sources.get(id)).map(|kept| (kept.source.title().to_string(), kept.entry.stock))
        };
        let Some((title, stock)) = kept else { return };
        let stays = "What you installed from it stays in your agents.";
        let goes = if stock {
            "It comes with the app and does not come back on its own once removed."
        } else {
            "The Store stops offering what it lists."
        };
        let question = Question {
            title: format!("Remove {title}?"),
            detail: format!("{goes} {stays}"),
            yes: "Remove".into(),
            asked: true,
            ..Question::default()
        };
        let id = id.to_string();
        self.asking(
            format!("source-remove {id}"),
            tab,
            question,
            move |store, given| given.map_or(Ok(()), |_| store.remove_source(&id)),
            None,
        );
    }

    /// The person is asked for a repository to add as a source, and with a yes it is read.
    pub(super) fn ask_add_source(&self, tab: &Arc<dyn Tab>) {
        let question = Question {
            title: "Add a Source".into(),
            detail: "A GitHub repository that holds a plugin marketplace or a folder of skills. \
                     What it lists counts as community: nobody reviewed it for you."
                .into(),
            fields: vec![Field {
                name: "repo".into(),
                label: "Repository, or its address".into(),
                hint: "owner/repo".into(),
                secret: false,
                required: true,
            }],
            yes: "Add".into(),
            asked: true,
            ..Question::default()
        };
        self.asking(
            "source-add".into(),
            tab,
            question,
            |store, given| {
                let Some(given) = given else { return Ok(()) };
                store.add_source(given.get("repo").map_or("", String::as_str))
            },
            None,
        );
    }

    /// `question` asked in `tab`'s window off this thread, unless the one named `key` is up
    /// already; `then` takes the person's answer, and what it could not do the page is told.
    /// `waits`, what an agent reads as waited for until they answer.
    fn asking(
        &self,
        key: String,
        tab: &Arc<dyn Tab>,
        question: Question,
        then: impl FnOnce(&Store, Option<Given>) -> Result<(), String> + Send + 'static,
        waits: Option<Value>,
    ) {
        {
            let mut state = crate::lock(&self.state);
            if !state.asking.insert(key.clone()) {
                return;
            }
            if waits.is_some() {
                state.asked = waits;
            }
        }
        let tab = Arc::clone(tab);
        self.go("store-ask", move |store| {
            let given = tab.ask(&question);
            crate::lock(&store.state).asking.remove(&key);
            if let Err(why) = then(&store, given) {
                tab.paint(&super::page::frame(
                    "said",
                    &json!({"said": why, "busy": false}),
                ));
            }
            store.tell("begun", &store.begun(false));
        });
    }
}

/// What a listing needs of the person as a field of the dialog: its name, what it is for, and
/// whether going on waits for it (what has a default does not).
fn field(need: &crate::catalog::Need) -> Field {
    let optional = if need.required { "" } else { " (optional)" };
    let about = if need.about.is_empty() {
        String::new()
    } else {
        format!(": {}", need.about)
    };
    Field {
        name: need.name.clone(),
        label: format!("{}{optional}{about}", need.name),
        hint: need.default.clone(),
        secret: need.secret,
        required: need.required && need.default.is_empty(),
    }
}
