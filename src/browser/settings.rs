//! The person's settings for the window: its appearance (theme, how much of the desktop shows
//! through, the fonts), where what an agent opens lands, and the shortcuts they changed. Kept in `settings.json` under the
//! config directory, read once, written whole on each change; a file that does not read is
//! the defaults (and left as it is until the person changes something, when it is kept beside as
//! `settings.json.bad`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::shortcuts::{self, Chord};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// How much of the desktop the glass lets through (macOS): none (an opaque coat) to the most.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Transparency {
    Off,
    Low,
    #[default]
    Standard,
    High,
}

/// The fonts offered, each a CSS stack that falls back to the system's.
const UI_FONTS: [(&str, &str, &str); 4] = [
    (
        "system",
        "System",
        "-apple-system,BlinkMacSystemFont,system-ui,sans-serif",
    ),
    (
        "rounded",
        "Rounded",
        "ui-rounded,-apple-system,system-ui,sans-serif",
    ),
    ("inter", "Inter", "Inter,-apple-system,system-ui,sans-serif"),
    (
        "helvetica",
        "Helvetica Neue",
        "\"Helvetica Neue\",Helvetica,Arial,sans-serif",
    ),
];
const CODE_FONTS: [(&str, &str, &str); 4] = [
    (
        "system",
        "System Mono",
        "ui-monospace,SFMono-Regular,Menlo,monospace",
    ),
    (
        "jetbrains",
        "JetBrains Mono",
        "\"JetBrains Mono\",ui-monospace,Menlo,monospace",
    ),
    (
        "fira",
        "Fira Code",
        "\"Fira Code\",ui-monospace,Menlo,monospace",
    ),
    ("menlo", "Menlo", "Menlo,Monaco,monospace"),
];
/// The terminals' font size, in points.
const SIZES: std::ops::RangeInclusive<u8> = 10..=20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Settings {
    pub theme: Theme,
    pub transparency: Transparency,
    pub ui_font: String,
    pub code_font: String,
    pub font_size: u8,
    pub updates: super::updates::Updates,
    /// The shortcuts the person changed, by id: a chord, or none (taken off).
    pub shortcuts: BTreeMap<String, Option<String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            transparency: Transparency::Standard,
            ui_font: "system".into(),
            code_font: "system".into(),
            font_size: 13,
            updates: super::updates::Updates::Auto,
            shortcuts: BTreeMap::new(),
        }
    }
}

fn stack(fonts: &[(&str, &str, &'static str)], id: &str) -> &'static str {
    (fonts.iter().find(|(at, _, _)| *at == id))
        .or_else(|| fonts.first())
        .map_or("system-ui", |(_, _, stack)| stack)
}

impl Settings {
    /// What the window's own pages are told: the fonts, as CSS, and each shortcut's chord as
    /// the system draws it, for the buttons' hints.
    pub(super) fn look(&self) -> Value {
        let keys: serde_json::Map<String, Value> = shortcuts::resolved(&self.shortcuts)
            .into_iter()
            .filter_map(|(shortcut, chord)| Some((shortcut.id.to_string(), chord?.shown().into())))
            .collect();
        json!({
            "ui": stack(&UI_FONTS, &self.ui_font),
            "mono": stack(&CODE_FONTS, &self.code_font),
            "size": self.font_size,
            "keys": keys,
        })
    }

    /// `change`, an object of the page's (`{"theme": "dark"}`, `{"shortcut": [id, chord]}`,
    /// `{"reset": id}`, `{"reset": "all"}`), made to these settings; what was wrong with it.
    pub(super) fn change(&mut self, change: &Value) -> Result<(), String> {
        let text = |name: &str| change.get(name).and_then(Value::as_str);
        if let Some(theme) = change.get("theme") {
            self.theme = serde_json::from_value(theme.clone()).map_err(|why| why.to_string())?;
        }
        if let Some(level) = change.get("transparency") {
            self.transparency =
                serde_json::from_value(level.clone()).map_err(|why| why.to_string())?;
        }
        if let Some(updates) = change.get("updates") {
            self.updates =
                serde_json::from_value(updates.clone()).map_err(|why| why.to_string())?;
        }
        if let Some(font) = text("ui_font") {
            if !UI_FONTS.iter().any(|(id, _, _)| *id == font) {
                return Err(format!("no font {font}"));
            }
            self.ui_font = font.to_string();
        }
        if let Some(font) = text("code_font") {
            if !CODE_FONTS.iter().any(|(id, _, _)| *id == font) {
                return Err(format!("no font {font}"));
            }
            self.code_font = font.to_string();
        }
        if let Some(size) = change.get("font_size").and_then(Value::as_u64) {
            let size = u8::try_from(size).unwrap_or(u8::MAX);
            self.font_size = size.clamp(*SIZES.start(), *SIZES.end());
        }
        if let Some([id, chord]) = change
            .get("shortcut")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
        {
            let id = id.as_str().ok_or("a shortcut is named by its id")?;
            let known = shortcuts::SHORTCUTS.iter().find(|s| s.id == id);
            let known = known.ok_or_else(|| format!("no shortcut {id}"))?;
            let chord = match chord.as_str() {
                Some(keys) => {
                    Some(Chord::parse(keys).ok_or_else(|| format!("{keys} is no chord"))?)
                }
                None => None,
            };
            // A row of places takes modifiers with a digit; a single shortcut, one key.
            let row = known.keys.ends_with("+digit");
            if let Some(chord) = &chord
                && (chord.key == "digit") != row
            {
                return Err(match row {
                    true => format!("{} takes modifiers with a digit", known.title),
                    false => format!("{} takes one key, not a row of digits", known.title),
                });
            }
            match chord {
                // Its own default again: nothing to keep.
                Some(chord) if Chord::parse(known.keys).as_ref() == Some(&chord) => {
                    self.shortcuts.remove(id);
                }
                chord => {
                    self.shortcuts
                        .insert(id.to_string(), chord.as_ref().map(Chord::text));
                }
            }
        }
        match text("reset") {
            Some("all") => self.shortcuts.clear(),
            Some(id) => drop(self.shortcuts.remove(id)),
            None => {}
        }
        Ok(())
    }

    /// The settings page's data: the settings and what each of the appearance's is by default
    /// (for its reset), the choices, and every shortcut with its chord, whether the person
    /// changed it, and whether another has the same chord.
    pub(super) fn page(&self) -> Value {
        let default = Self::default();
        let resolved = shortcuts::resolved(&self.shortcuts);
        let clashes = shortcuts::clashes(&resolved);
        let keys: Vec<Value> = resolved
            .iter()
            .map(|(shortcut, chord)| {
                json!({
                    "id": shortcut.id,
                    "title": shortcut.title,
                    "group": shortcut.group,
                    "keys": chord.as_ref().map(Chord::text),
                    "shown": chord.as_ref().map(Chord::shown),
                    "default": Chord::parse(shortcut.keys).map(|chord| chord.shown()),
                    "row": shortcut.keys.ends_with("+digit"),
                    "changed": self.shortcuts.contains_key(shortcut.id),
                    "clash": clashes.contains(&shortcut.id),
                })
            })
            .collect();
        let fonts = |fonts: &[(&str, &str, &str)]| -> Vec<Value> {
            (fonts.iter())
                .map(|(id, name, _)| json!({"id": id, "name": name}))
                .collect()
        };
        json!({
            "settings": self,
            "defaults": {
                "theme": default.theme,
                "transparency": default.transparency,
                "ui_font": default.ui_font,
                "code_font": default.code_font,
                "size": default.font_size,
                "updates": default.updates,
            },
            "ui_fonts": fonts(&UI_FONTS),
            "code_fonts": fonts(&CODE_FONTS),
            "sizes": [SIZES.start(), SIZES.end()],
            // Window transparency is the window's glass, which only macOS has.
            "glass": cfg!(target_os = "macos"),
            "shortcuts": keys,
        })
    }

    /// Read from `file`; the defaults where it is missing or does not read.
    pub(super) fn read(file: &Path) -> Self {
        match std::fs::read_to_string(file) {
            Ok(text) => serde_json::from_str(&text).map_or_else(
                |why| {
                    log::warn!("{}: {why}: the defaults are used", file.display());
                    Self::default()
                },
                |mut read: Self| {
                    // A size by hand or from another version: held to what Settings offers.
                    read.font_size = read.font_size.clamp(*SIZES.start(), *SIZES.end());
                    read
                },
            ),
            Err(_) => Self::default(),
        }
    }

    /// Written to `file`, whole: a new file moved over the old, so a crash leaves one or the
    /// other.
    pub(super) fn write(&self, file: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|why| why.to_string())?;
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|why| format!("{}: {why}", dir.display()))?;
        }
        // A file that did not read (a typo, another version's) is kept aside, not written over.
        let unread = std::fs::read_to_string(file)
            .is_ok_and(|text| serde_json::from_str::<Self>(&text).is_err());
        if unread {
            let _ = std::fs::rename(file, file.with_extension("json.bad"));
        }
        let new = file.with_extension("json.new");
        std::fs::write(&new, text + "\n").map_err(|why| format!("{}: {why}", new.display()))?;
        std::fs::rename(&new, file).map_err(|why| format!("{}: {why}", file.display()))
    }
}

/// The file the settings are kept in; none without a home.
fn file() -> Option<PathBuf> {
    crate::paths::config().map(|dir| dir.join("settings.json"))
}

fn kept() -> &'static Mutex<Settings> {
    static KEPT: OnceLock<Mutex<Settings>> = OnceLock::new();
    KEPT.get_or_init(|| Mutex::new(file().map(|file| Settings::read(&file)).unwrap_or_default()))
}

/// The settings now.
pub(super) fn now() -> Settings {
    crate::lock(kept()).clone()
}

/// `change` made and kept; the settings after it.
pub(super) fn change(change: &Value) -> Result<Settings, String> {
    let mut settings = crate::lock(kept());
    let mut changed = settings.clone();
    changed.change(change)?;
    if changed != *settings {
        if let Some(file) = file() {
            changed.write(&file)?;
        }
        *settings = changed;
    }
    Ok(settings.clone())
}

/// The script that gives one of the window's own pages the fonts (`--ui-font`, `--mono`,
/// `--mono-size` on its root), whether transparency is reduced (`data-reduced`, which WebKit's
/// own media query does not say) and tells it (`desk-look`): at its start and on each change. The
/// look also says whether a screen reader runs (`reader`), for a page that builds its own
/// reading tree only then.
pub(super) fn look_script(settings: &Settings) -> String {
    let mut look = settings.look();
    look["reader"] = super::frame::screen_reader().into();
    format!(
        "(()=>{{const l={look};window.__deskLook=l;const put=()=>{{const r=document.documentElement,s=r.style;\
         s.setProperty('--ui-font',l.ui);s.setProperty('--mono',l.mono);s.setProperty('--mono-size',l.size+'px');\
         r.toggleAttribute('data-reduced',{reduced});\
         dispatchEvent(new CustomEvent('desk-look',{{detail:l}}))}};\
         if(document.documentElement)put();else document.addEventListener('DOMContentLoaded',put,{{once:true}})}})()",
        look = look,
        reduced = super::frame::reduced()
    )
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
