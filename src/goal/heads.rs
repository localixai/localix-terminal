//! The heads of one step's question, and what each offered: the targets, the (field, value)
//! pairs, the options, the waits, the keys and the addresses a step may name.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::goal::offer::{choices, dropdown};
use crate::model::Question;
use crate::view::{Element, KEYS};

/// The head that names a key, and the one that names an address.
pub(crate) const KEY_HEAD: &str = "key";
pub(crate) const ADDRESS_HEAD: &str = "address";
/// The waits the model can choose, in seconds.
const WAITS: [&str; 7] = ["1", "2", "3", "5", "10", "20", "30"];

/// The heads asked so far, what each offered, and what a type or select key stands for.
#[derive(Default)]
pub(crate) struct Heads {
    pub(crate) questions: BTreeMap<String, Question>,
    pub(crate) offered: BTreeMap<String, Map<String, Value>>,
    pub(crate) pairs: BTreeMap<String, (String, String)>,
}

impl Heads {
    pub(crate) fn add(&mut self, key: &str, instructions: &str, criteria: Map<String, Value>) {
        self.questions.insert(
            key.to_string(),
            Question::Choice {
                instructions: json!(instructions),
                criteria: criteria.clone(),
            },
        );
        self.offered.insert(key.to_string(), criteria);
    }

    /// Each field with each value it may take, the pair kept under its key.
    pub(crate) fn typed(
        &mut self,
        typing: Vec<(String, String, String)>,
        values: &[String],
    ) -> Map<String, Value> {
        let mut criteria = Map::new();
        for (key, reference, text) in typing {
            let index: usize = key
                .rsplit("=v")
                .next()
                .and_then(|i| i.parse().ok())
                .unwrap_or(1);
            self.pairs
                .insert(key.clone(), (reference, values[index - 1].clone()));
            criteria.insert(key, json!(text));
        }
        criteria
    }

    /// Every option of every dropdown, the pair kept under its key.
    pub(crate) fn options(&mut self, elements: &[&Element]) -> Map<String, Value> {
        let mut options = Map::new();
        for element in elements.iter().filter(|e| dropdown(e)) {
            for (i, option) in choices(element) {
                let key = format!("{}:{}", element.reference, i + 1);
                options.insert(
                    key.clone(),
                    json!(format!("{} → {}", element.describe(), option.label)),
                );
                self.pairs
                    .insert(key, (element.reference.clone(), option.value.clone()));
            }
        }
        options
    }

    /// The waits the model can choose; each ends early when the view changes.
    pub(crate) fn waits() -> Map<String, Value> {
        WAITS
            .iter()
            .map(|s| {
                (
                    s.to_string(),
                    json!(format!(
                        "{s} seconds at most; it ends as soon as the view changes"
                    )),
                )
            })
            .collect()
    }

    /// Every key a step may press, by its name (`view::KEYS`, the same a call's `keys` take).
    pub(crate) fn keys() -> Map<String, Value> {
        KEYS.iter()
            .map(|key| (key.to_string(), json!(format!("the {key} key"))))
            .collect()
    }

    /// Each address the step may open, the address kept under its key.
    pub(crate) fn addresses(&mut self, addresses: &[&str]) -> Map<String, Value> {
        let mut criteria = Map::new();
        for (i, url) in addresses.iter().enumerate() {
            let key = format!("a{}", i + 1);
            self.pairs
                .insert(key.clone(), (String::new(), url.to_string()));
            criteria.insert(key, json!(format!("go to {url}")));
        }
        criteria
    }
}
