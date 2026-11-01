//! Opening tabs: one or several at once, each a page or a terminal, each owned from the moment it
//! exists, a `goal` working in it when it has one.

use serde_json::Value;

use super::actions::{Kind, parse};
use super::args::{strings, text};
use super::{Call, Desk, Failure, Output};

impl Desk {
    /// Open each tab asked for; one that fails says why, and the others still open.
    #[expect(
        clippy::needless_collect,
        reason = "every tab's thread starts before the first is joined"
    )]
    pub(super) fn open(&self, call: &Call, args: &Value) -> Result<Output, Failure> {
        let asked: Vec<&Value> = match args.get("tabs").and_then(Value::as_array) {
            Some(tabs) => tabs.iter().collect(),
            None => vec![args],
        };
        let opened: Vec<Result<(String, Output), Failure>> = std::thread::scope(|scope| {
            let opening: Vec<_> = asked
                .iter()
                .map(|spec| {
                    let thread = std::thread::Builder::new().name("desk-tab-open".into());
                    thread.spawn_scoped(scope, move || self.open_one(call, spec))
                })
                .collect();
            let joined = opening.into_iter().map(|started| match started {
                Ok(thread) => thread
                    .join()
                    .unwrap_or_else(|_| Err("opening it failed".to_string().into())),
                Err(why) => Err(format!("it could not start (no thread: {why})").into()),
            });
            joined.collect()
        });
        let (mut reports, mut states, mut shot) = (vec![], vec![], None);
        for opened in opened {
            match opened {
                Ok((label, out)) => {
                    reports.push(format!("{label}: {}", out.report));
                    if let Some(state) = out.state {
                        states.push(format!("### {label}\n{state}"));
                    }
                    shot = out.shot.or(shot);
                }
                Err(Failure::Fatal(why)) if reports.is_empty() => return Err(Failure::Fatal(why)),
                Err(why) => reports.push(format!("not opened: {}", why.message())),
            }
        }
        Ok(Output {
            report: reports.join("\n"),
            state: (!states.is_empty()).then(|| states.join("\n\n")),
            shot,
        })
    }

    /// One tab of an `open`, owned from the moment it exists; its goal, if it has one, works in it.
    fn open_one(&self, call: &Call, spec: &Value) -> Result<(String, Output), Failure> {
        let kind = match text(spec, "kind") {
            Some("browser") => Kind::Web,
            Some("terminal") => Kind::Term,
            Some(other) => {
                return Err(format!("no kind of tab `{other}`: browser or terminal").into());
            }
            None if text(spec, "command").is_some() => Kind::Term,
            None => Kind::Web,
        };
        let mark = self.told_mark();
        let (key, mut out) = match kind {
            Kind::Web => {
                let (id, out) = self.open_web(call, spec)?;
                (Some(format!("w{id}")), out)
            }
            Kind::Term => {
                let (id, out) = self.open_term(call, spec)?;
                (id.map(|id| format!("t{id}")), out)
            }
        };
        let Some(key) = key else {
            // The program ended within the wait: no tab, and what it printed.
            return Ok((kind.name().to_string(), out));
        };
        {
            let mut registry = crate::lock(&self.registry);
            registry.own(&key, call.owner);
            if let (Kind::Web, Some(browser)) = (kind, &self.browser) {
                browser.group(&key, &registry.group_of(call.owner));
            }
        }
        let tab = parse(&key);
        if let (Some(goal), Some(tab)) = (text(spec, "goal"), &tab) {
            let answering = match kind {
                Kind::Web => self.answering(tab.id, spec)?,
                Kind::Term => None,
            };
            let helped = self.goal_on(call, tab, goal, &strings(spec, "values"));
            drop(answering);
            match helped {
                Ok(report) => out.report = format!("{}\n{report}", out.report),
                Err(why) => {
                    out.report = format!("{}\nthe goal failed: {}", out.report, why.message())
                }
            }
            out.state = self
                .read(tab)
                .ok()
                .and_then(|read| read.state)
                .or(out.state);
        }
        if let Some(told) =
            (tab.filter(|tab| tab.kind == Kind::Web)).and_then(|tab| self.told(tab.id, mark))
        {
            out.report = format!(
                "{}

{told}",
                out.report
            );
        }
        Ok((format!("{key} ({})", kind.name()), out))
    }
}
