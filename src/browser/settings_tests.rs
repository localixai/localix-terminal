//! Tests of `settings`.

use serde_json::json;

use super::*;
use crate::testing::Scratch;

#[test]
fn a_change_is_checked_and_a_shortcut_back_on_its_default_is_not_kept() {
    let mut settings = Settings::default();
    settings
        .change(&json!({"theme": "dark", "transparency": "high", "font_size": 99}))
        .unwrap();
    assert_eq!(settings.theme, Theme::Dark);
    assert_eq!(settings.transparency, Transparency::High);
    assert_eq!(settings.font_size, 20);
    assert!(settings.change(&json!({"theme": "neon"})).is_err());
    assert!(settings.change(&json!({"ui_font": "comic"})).is_err());

    settings
        .change(&json!({"shortcut": ["reload", "cmd+shift+r"]}))
        .unwrap();
    settings
        .change(&json!({"shortcut": ["history", null]}))
        .unwrap();
    assert_eq!(settings.shortcuts["reload"].as_deref(), Some("shift+cmd+r"));
    assert_eq!(settings.shortcuts["history"], None);
    assert!(
        settings
            .change(&json!({"shortcut": ["reload", "r"]}))
            .is_err()
    );
    assert!(
        settings
            .change(&json!({"shortcut": ["nothing", "cmd+g"]}))
            .is_err()
    );
    settings
        .change(&json!({"shortcut": ["reload", "cmd+r"]}))
        .unwrap();
    assert!(!settings.shortcuts.contains_key("reload"));
    settings.change(&json!({"reset": "history"})).unwrap();
    assert!(settings.shortcuts.is_empty());
    settings
        .change(&json!({"shortcut": ["send", "cmd+l"]}))
        .unwrap();
    let page = settings.page();
    let send = (page["shortcuts"].as_array().unwrap().iter())
        .find(|row| row["id"] == "send")
        .unwrap();
    assert_eq!(send["clash"], true);
    assert_eq!(send["changed"], true);
    settings.change(&json!({"reset": "all"})).unwrap();
    assert!(settings.shortcuts.is_empty());
}

#[test]
fn settings_outlive_the_run_and_a_file_that_does_not_read_is_the_defaults() {
    let scratch = Scratch::new();
    let file = scratch.path().join("llx-desk").join("settings.json");
    assert_eq!(Settings::read(&file), Settings::default());
    let mut settings = Settings::default();
    settings
        .change(&json!({"code_font": "jetbrains", "theme": "light"}))
        .unwrap();
    settings.write(&file).unwrap();
    assert_eq!(Settings::read(&file), settings);
    std::fs::write(&file, "{ not json").unwrap();
    assert_eq!(Settings::read(&file), Settings::default());
    // A file of an older run, with fields this one does not know, still reads.
    std::fs::write(&file, r#"{"theme": "dark", "zoom": 2}"#).unwrap();
    assert_eq!(Settings::read(&file).theme, Theme::Dark);
}

#[test]
fn the_pages_are_told_the_fonts() {
    let mut settings = Settings::default();
    settings
        .change(&json!({"code_font": "fira", "font_size": 15}))
        .unwrap();
    let script = look_script(&settings);
    assert!(
        script.contains(r#"\"Fira Code\""#) && script.contains(r#""size":15"#),
        "{script}"
    );
    assert!(!script.contains("</script"));
}

#[test]
fn a_row_of_places_takes_a_digit_and_a_single_shortcut_does_not() {
    let mut settings = Settings::default();
    settings
        .change(&json!({"shortcut": ["tab-place", "ctrl+digit"]}))
        .unwrap();
    assert_eq!(
        settings.shortcuts["tab-place"].as_deref(),
        Some("ctrl+digit")
    );
    assert!(
        settings
            .change(&json!({"shortcut": ["tab-place", "cmd+k"]}))
            .is_err()
    );
    assert!(
        settings
            .change(&json!({"shortcut": ["reload", "cmd+digit"]}))
            .is_err()
    );
}

#[test]
fn the_glass_levels_are_even_steps_from_opaque_to_clearest() {
    for standard in [
        crate::browser::frame::coat(true).1,
        crate::browser::frame::coat(false).1,
    ] {
        let levels = [
            Transparency::Off,
            Transparency::Low,
            Transparency::Standard,
            Transparency::High,
        ]
        .map(|level| level.coat(standard));
        assert!((levels[0] - 1.0).abs() < f64::EPSILON, "off is opaque");
        let steps: Vec<f64> = levels.windows(2).map(|pair| pair[0] - pair[1]).collect();
        assert!(
            steps.iter().all(|step| *step > 0.1),
            "each clearer: {levels:?}"
        );
        let (least, most) = (
            steps.iter().copied().fold(1.0, f64::min),
            steps.iter().copied().fold(0.0, f64::max),
        );
        assert!(most - least < 0.12, "even steps: {levels:?}");
    }
}
