//! A kind for tests: it keeps what it was told, and says it back.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::{About, Kind, Said, Tab};

pub static ECHO: About = About {
    id: "echo",
    letter: 'x',
    title: "Echo",
    holds: "What it was told",
    glyph: "M3 8h10",
    fields: &[(
        "say",
        r#"{"type": "string", "description": "echo: what to say back."}"#,
    )],
    guide: "Says back what it is told with `say`.",
    instead: "An echo is told with `say`.",
    closed: "The person closed the echo's tab.",
    keys: "cmd+alt+e",
};

/// What the echo heard, in order: `loaded`, `call …`, `posted …`.
#[derive(Default)]
pub struct Echo {
    pub heard: Mutex<Vec<String>>,
    /// What it says of itself when that is not [`ECHO`].
    pub about: Option<&'static About>,
}

impl Echo {
    /// An echo listed as this thread's only kind.
    pub fn installed() -> Arc<Self> {
        let echo = Arc::new(Self::default());
        super::install(vec![Arc::clone(&echo) as Arc<dyn Kind>]);
        echo
    }

    /// An echo that says `about` of itself.
    pub fn of(about: &'static About) -> Arc<dyn Kind> {
        Arc::new(Self {
            about: Some(about),
            ..Self::default()
        })
    }

    pub fn heard(&self) -> Vec<String> {
        crate::lock(&self.heard).clone()
    }
}

impl Kind for Echo {
    fn about(&self) -> &'static About {
        self.about.unwrap_or(&ECHO)
    }

    fn page(&self, title: &str) -> String {
        format!("<title>{title}</title>")
    }

    fn loaded(&self, tab: &Arc<dyn Tab>) {
        crate::lock(&self.heard).push("loaded".into());
        tab.paint("draw()");
    }

    fn call(&self, args: &Value, _: &Arc<dyn Tab>) -> Result<Said, String> {
        let say = args.get("say").and_then(Value::as_str);
        let say = say.ok_or("`say` is needed")?;
        crate::lock(&self.heard).push(format!("call {say}"));
        Ok(Said {
            report: format!("said {say}"),
            state: Some(self.text()),
        })
    }

    fn posted(&self, body: &str, _: &Arc<dyn Tab>) {
        crate::lock(&self.heard).push(format!("posted {body}"));
    }

    fn text(&self) -> String {
        format!("echo: {}", self.heard().join(", "))
    }

    fn summary(&self) -> String {
        format!("{} heard", self.heard().len())
    }
}
