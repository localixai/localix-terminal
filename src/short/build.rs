//! A step's words read by its verb's row (its tab, its words, its flags), and the action each verb
//! builds of them.
#![expect(
    clippy::unnecessary_wraps,
    reason = "every builder has the one signature of the table's column, though some never fail"
)]

use serde_json::{Map, Value, json};

use super::Verb;

/// Whether a verb's first word may name its tab.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    /// It never does.
    No,
    /// It does when it looks like one and what the verb needs still follows it.
    Maybe,
    /// It always does.
    Must,
}

/// A step's words as its verb reads them.
pub(super) struct Given<'a> {
    verb: &'static Verb,
    tab: Option<String>,
    words: Vec<String>,
    /// The first of `words` that came after `--`: it and those after are values as written.
    literal: usize,
    flags: Vec<(&'static str, Option<String>)>,
    stdin: &'a mut dyn FnMut() -> Result<String, String>,
}

/// `w1`, `t2`, `b1`: a tab's key, never a ref (`e12`).
fn is_tab(word: &str) -> bool {
    let mut chars = word.chars();
    let letter = chars.next().filter(|c| c.is_ascii_lowercase() && *c != 'e');
    letter.is_some() && !chars.as_str().is_empty() && chars.all(|c| c.is_ascii_digit())
}

impl<'a> Given<'a> {
    /// `rest`, the words after the verb, read by its row: flags by name, the tab first.
    pub(super) fn read(
        verb: &'static Verb,
        rest: &[String],
        stdin: &'a mut dyn FnMut() -> Result<String, String>,
    ) -> Result<Self, String> {
        let mut given = Self {
            verb,
            tab: None,
            words: vec![],
            literal: usize::MAX,
            flags: vec![],
            stdin,
        };
        let mut rest = rest.iter();
        while let Some(word) = rest.next() {
            if given.literal != usize::MAX {
                given.words.push(word.clone());
            } else if word == "--" {
                given.literal = given.words.len();
            } else if let Some(name) = word.strip_prefix("--") {
                let flag = (verb.flags.iter()).find(|flag| flag.trim_end_matches('=') == name);
                let flag = flag.ok_or_else(|| given.usage(&format!("no option `{word}`")))?;
                let value = match flag.ends_with('=') {
                    true => Some(
                        rest.next()
                            .ok_or_else(|| given.usage(&format!("`{word}` needs a value")))?
                            .clone(),
                    ),
                    false => None,
                };
                given.flags.push((flag.trim_end_matches('='), value));
            } else {
                given.words.push(word.clone());
            }
        }
        let first_is_tab = given.words.first().is_some_and(|word| is_tab(word));
        let takes_tab = match verb.tab {
            Tab::No => false,
            Tab::Maybe => first_is_tab && given.literal > 0 && given.words.len() > verb.needs,
            Tab::Must if first_is_tab => true,
            Tab::Must => return Err(given.usage("needs a tab: `w1`, `t2`")),
        };
        if takes_tab {
            given.tab = Some(given.words.remove(0));
            given.literal = given.literal.saturating_sub(1);
        }
        if given.words.len() < verb.needs {
            return Err(given.usage("needs more"));
        }
        Ok(given)
    }

    /// `why`, with how the command is written.
    pub(super) fn usage(&self, why: &str) -> String {
        format!("{}: {why}: llxterm {}", self.verb.name, self.verb.synopsis)
    }

    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(flag, _)| *flag == name)
    }

    fn flag(&self, name: &str) -> Option<&str> {
        (self.flags.iter())
            .find(|(flag, _)| *flag == name)
            .and_then(|(_, value)| value.as_deref())
    }

    fn number(&self, name: &str) -> Result<Option<u64>, String> {
        let Some(value) = self.flag(name) else {
            return Ok(None);
        };
        (value.parse().map(Some))
            .map_err(|_not| self.usage(&format!("`--{name}` takes a number, not `{value}`")))
    }

    /// The words from `from` on as one text, joined by spaces; a lone `-` before `--` is stdin.
    fn text(&mut self, from: usize) -> Result<Option<String>, String> {
        let words = self.words.get(from..).unwrap_or_default();
        if let [one] = words
            && one == "-"
            && from < self.literal
        {
            let read = (self.stdin)()?;
            return Ok(Some(read.strip_suffix('\n').unwrap_or(&read).to_string()));
        }
        Ok((!words.is_empty()).then(|| words.join(" ")))
    }

    /// The action `name` on the tab given, with the page's flags.
    fn action(&self, name: &str) -> Map<String, Value> {
        let mut action = Map::from_iter([("action".to_string(), json!(name))]);
        if let Some(tab) = &self.tab {
            action.insert("tab".into(), json!(tab));
        }
        for (flag, field) in [("dialog", "dialog"), ("answer", "dialog_text")] {
            if let Some(value) = self.flag(flag) {
                action.insert(field.into(), json!(value));
            }
        }
        action
    }

    fn interact(&self, fields: &[(&str, Option<Value>)]) -> Value {
        let mut action = self.action("interact");
        for (name, value) in fields {
            if let Some(value) = value {
                action.insert((*name).into(), value.clone());
            }
        }
        Value::Object(action)
    }
}

pub(super) fn open(given: &mut Given) -> Result<Value, String> {
    let mut action = given.action("open");
    action.insert("url".into(), json!(given.text(0)?));
    Ok(Value::Object(action))
}

pub(super) fn term(given: &mut Given) -> Result<Value, String> {
    let mut action = given.action("open");
    action.insert("kind".into(), json!("terminal"));
    let fields = [
        ("command", given.text(0)?.map(Value::from)),
        ("workdir", given.flag("in").map(Value::from)),
        ("wait_for", given.flag("wait").map(Value::from)),
        ("yield_ms", given.number("ms")?.map(Value::from)),
        ("beside", given.flag("beside").map(Value::from)),
        ("side", given.has("down").then(|| json!("down"))),
    ];
    action.extend(
        fields
            .into_iter()
            .filter_map(|(name, value)| Some((name.to_string(), value?))),
    );
    Ok(Value::Object(action))
}

pub(super) fn edit(given: &mut Given) -> Result<Value, String> {
    let mut action = given.action("open");
    action.insert("kind".into(), json!("file"));
    action.insert("path".into(), json!(given.text(0)?));
    if let Some(line) = given.number("line")? {
        action.insert("line".into(), json!(line));
    }
    Ok(Value::Object(action))
}

pub(super) fn goto(given: &mut Given) -> Result<Value, String> {
    let url = given.text(0)?.map(Value::from);
    Ok(given.interact(&[("url", url)]))
}

pub(super) fn click(given: &mut Given) -> Result<Value, String> {
    let clicks = match given.words.as_slice() {
        [one] => json!(one),
        many => json!(many),
    };
    Ok(given.interact(&[("click", Some(clicks))]))
}

pub(super) fn fill(given: &mut Given) -> Result<Value, String> {
    let mut fields = Map::new();
    for pair in given.words.chunks(2) {
        match pair {
            [at, text] => fields.insert(at.clone(), json!(text)),
            [at] => return Err(given.usage(&format!("{at} has no text"))),
            _ => None,
        };
    }
    Ok(given.interact(&[("fill", Some(Value::Object(fields)))]))
}

pub(super) fn select(given: &mut Given) -> Result<Value, String> {
    let at = given.words.first().cloned().unwrap_or_default();
    let option = given.text(1)?.unwrap_or_default();
    Ok(given.interact(&[("select", Some(json!({at: option})))]))
}

pub(super) fn press(given: &mut Given) -> Result<Value, String> {
    let delay = given.number("delay")?.map(Value::from);
    Ok(given.interact(&[("keys", Some(json!(given.words))), ("key_delay_ms", delay)]))
}

pub(super) fn type_in(given: &mut Given) -> Result<Value, String> {
    let input = given.text(0)?.map(Value::from);
    let enter = given.has("enter").then(|| json!(["Enter"]));
    Ok(given.interact(&[("input", input), ("keys", enter)]))
}

pub(super) fn wait(given: &mut Given) -> Result<Value, String> {
    let wanted = given.text(0)?.map(Value::from);
    let most = given.number("ms")?.map(Value::from);
    Ok(given.interact(&[("wait_for", wanted), ("yield_ms", most)]))
}

pub(super) fn read(given: &mut Given) -> Result<Value, String> {
    let offset = given.number("from")?.map(Value::from);
    Ok(given.interact(&[
        ("read", given.has("text").then_some(json!(true))),
        ("offset", offset),
    ]))
}

pub(super) fn eval(given: &mut Given) -> Result<Value, String> {
    let script = given.text(0)?.map(Value::from);
    Ok(given.interact(&[("script", script)]))
}

pub(super) fn size(given: &mut Given) -> Result<Value, String> {
    let size = given.text(0)?.map(Value::from);
    Ok(given.interact(&[("size", size)]))
}

pub(super) fn ask(given: &mut Given) -> Result<Value, String> {
    let ask = given.text(0)?.map(Value::from);
    Ok(given.interact(&[("ask", ask)]))
}

pub(super) fn shot(given: &mut Given) -> Result<Value, String> {
    let mut action = given.action("screenshot");
    if let Some(selector) = given.flag("of") {
        action.insert("selector".into(), json!(selector));
    }
    Ok(Value::Object(action))
}

pub(super) fn show(given: &mut Given) -> Result<Value, String> {
    let mut action = given.action("show");
    match (given.has("zoom"), given.has("unzoom")) {
        (true, true) => return Err(given.usage("one of --zoom and --unzoom")),
        (true, false) => action.insert("zoom".into(), json!(true)),
        (false, true) => action.insert("zoom".into(), json!(false)),
        (false, false) => None,
    };
    Ok(Value::Object(action))
}

pub(super) fn close(given: &mut Given) -> Result<Value, String> {
    let mut action = given.action("close");
    action.insert("tabs".into(), json!(given.words));
    Ok(Value::Object(action))
}

pub(super) fn list(given: &mut Given) -> Result<Value, String> {
    Ok(Value::Object(given.action("list")))
}

/// `do` among other steps: it holds steps, and is none of them.
pub(super) fn nested(given: &mut Given) -> Result<Value, String> {
    Err(given.usage("holds the steps itself, and is never one of them"))
}
