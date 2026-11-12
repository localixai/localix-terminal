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
fn a_size_by_hand_is_held_to_the_sizes_and_a_file_that_did_not_read_is_kept_aside() {
    let scratch = Scratch::new();
    let file = scratch.path().join("llx-desk").join("settings.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, r#"{"font_size": 0}"#).unwrap();
    assert_eq!(Settings::read(&file).font_size, *SIZES.start());
    std::fs::write(&file, r#"{"font_size": 200}"#).unwrap();
    assert_eq!(Settings::read(&file).font_size, *SIZES.end());
    // Not written over: the person's file is still theirs, beside the new one.
    std::fs::write(&file, "{ not json").unwrap();
    Settings::default().write(&file).unwrap();
    assert_eq!(
        std::fs::read_to_string(file.with_extension("json.bad")).unwrap(),
        "{ not json"
    );
    assert_eq!(Settings::read(&file), Settings::default());
}

#[test]
fn the_page_is_told_what_each_appearance_is_by_default() {
    let mut settings = Settings::default();
    settings
        .change(&json!({"theme": "dark", "font_size": 17}))
        .unwrap();
    let defaults = &settings.page()["defaults"];
    assert_eq!(defaults["theme"], "system");
    assert_eq!(defaults["transparency"], "standard");
    assert_eq!(defaults["ui_font"], "system");
    assert_eq!(defaults["code_font"], "system");
    assert_eq!(defaults["size"], 13);
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
    // The pages are told whether transparency is reduced, as WebKit's own query does not.
    assert!(
        script.contains("toggleAttribute('data-reduced',"),
        "{script}"
    );
}

#[test]
fn the_pages_are_told_whether_a_screen_reader_is_running() {
    let script = look_script(&Settings::default());
    // The terminal builds its reading tree only for a person who uses one: it costs a row of
    // elements per line, and an agent's read of the page would take them for text.
    assert!(script.contains(r#""reader":false"#), "{script}");
}

#[test]
fn the_page_says_whether_the_window_has_glass_to_make_less_clear() {
    assert_eq!(
        Settings::default().page()["glass"],
        cfg!(target_os = "macos")
    );
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
fn a_setting_of_a_version_before_is_read_past() {
    // Where an agent's documents open was a choice once: they join a session now.
    let read: Settings = serde_json::from_str(r#"{"theme":"dark","agent_opens":"tab"}"#).unwrap();
    assert_eq!(read.theme, Theme::Dark);
}

#[test]
fn updates_are_looked_for_unless_the_person_chose_to_ask() {
    use crate::browser::updates::Updates;
    let mut settings = Settings::default();
    assert_eq!(settings.updates, Updates::Auto);
    assert_eq!(settings.page()["defaults"]["updates"], "auto");
    settings.change(&json!({"updates": "manual"})).unwrap();
    assert_eq!(settings.updates, Updates::Manual);
    assert_eq!(settings.page()["settings"]["updates"], "manual");
    assert!(settings.change(&json!({"updates": "nightly"})).is_err());
    // Settings kept before there was such a choice read as the default.
    let kept: Settings = serde_json::from_str(r#"{"theme": "dark"}"#).unwrap();
    assert_eq!(kept.updates, Updates::Auto);
}
