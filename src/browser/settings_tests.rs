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
    let file = scratch.path().join("llxterm").join("settings.json");
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
    let file = scratch.path().join("llxterm").join("settings.json");
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
    assert_eq!(defaults["theme"], "dark");
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

#[test]
fn neither_option_is_meta_unless_the_person_chose_one_and_the_terminals_are_told() {
    let mut settings = Settings::default();
    // Many layouts type @ [ ] { } | \ ~ with either Option: both keep typing by default.
    assert_eq!(settings.option_meta, OptionMeta::Off);
    assert_eq!(settings.page()["defaults"]["option_meta"], "off");
    assert!(look_script(&settings).contains(r#""option":"off""#));
    for (chosen, meant) in [
        ("left", OptionMeta::Left),
        ("right", OptionMeta::Right),
        ("both", OptionMeta::Both),
        ("off", OptionMeta::Off),
    ] {
        settings.change(&json!({"option_meta": chosen})).unwrap();
        assert_eq!(settings.option_meta, meant);
        assert_eq!(settings.page()["settings"]["option_meta"], chosen);
        assert!(look_script(&settings).contains(&format!(r#""option":"{chosen}""#)));
    }
    assert!(settings.change(&json!({"option_meta": "middle"})).is_err());
    let kept: Settings = serde_json::from_str(r#"{"theme": "dark"}"#).unwrap();
    assert_eq!(kept.option_meta, OptionMeta::Off);
}

#[test]
fn ligatures_join_unless_the_person_turned_them_off() {
    let mut settings = Settings::default();
    assert_eq!(settings.ligatures, Ligatures::On);
    assert!(settings.ligatures.on());
    assert_eq!(settings.page()["defaults"]["ligatures"], "on");
    settings.change(&json!({"ligatures": "off"})).unwrap();
    assert!(!settings.ligatures.on());
    assert_eq!(settings.page()["settings"]["ligatures"], "off");
    assert!(settings.change(&json!({"ligatures": "some"})).is_err());
    let kept: Settings = serde_json::from_str(r#"{"theme": "dark"}"#).unwrap();
    assert_eq!(
        kept.ligatures,
        Ligatures::On,
        "kept before there was such a choice"
    );
}

#[test]
fn agents_go_on_in_their_terminals_unless_the_person_turned_it_off() {
    use crate::browser::agents::Resuming;
    let mut settings = Settings::default();
    assert_eq!(settings.resume_agents, Resuming::On);
    assert_eq!(settings.page()["defaults"]["resume_agents"], "on");
    settings.change(&json!({"resume_agents": "off"})).unwrap();
    assert_eq!(settings.resume_agents, Resuming::Off);
    assert_eq!(settings.page()["settings"]["resume_agents"], "off");
    assert!(settings.change(&json!({"resume_agents": "later"})).is_err());
    let kept: Settings = serde_json::from_str(r#"{"theme": "dark"}"#).unwrap();
    assert_eq!(kept.resume_agents, Resuming::On);
}

#[test]
fn a_command_typed_out_of_sight_is_told_after_thirty_seconds_unless_the_person_chose_otherwise() {
    use std::time::Duration;
    let mut settings = Settings::default();
    assert_eq!(settings.command_notice, CommandNotice::After30s);
    assert_eq!(
        settings.command_notice.after(),
        Some(Duration::from_secs(30))
    );
    assert_eq!(settings.page()["defaults"]["command_notice"], "30s");
    for (chosen, after) in [
        ("off", None),
        ("10s", Some(10)),
        ("30s", Some(30)),
        ("1m", Some(60)),
        ("5m", Some(300)),
    ] {
        settings.change(&json!({"command_notice": chosen})).unwrap();
        assert_eq!(
            settings.command_notice.after(),
            after.map(Duration::from_secs)
        );
        assert_eq!(settings.page()["settings"]["command_notice"], chosen);
    }
    assert!(settings.change(&json!({"command_notice": "7s"})).is_err());
    assert!(settings.change(&json!({"command_notice": 30})).is_err());
    let kept: Settings = serde_json::from_str(r#"{"theme": "dark"}"#).unwrap();
    assert_eq!(kept.command_notice, CommandNotice::After30s);
}

#[test]
fn a_link_clicked_in_a_terminal_asks_where_it_opens_unless_the_person_chose_one_place() {
    let mut settings = Settings::default();
    assert_eq!(settings.open_links, OpenLinks::Ask);
    assert_eq!(settings.page()["defaults"]["open_links"], "ask");
    for (chosen, place) in [
        ("tab", OpenLinks::Tab),
        ("browser", OpenLinks::Browser),
        ("ask", OpenLinks::Ask),
    ] {
        settings.change(&json!({"open_links": chosen})).unwrap();
        assert_eq!(settings.open_links, place);
        assert_eq!(settings.page()["settings"]["open_links"], chosen);
    }
    assert!(settings.change(&json!({"open_links": "window"})).is_err());
    assert!(settings.change(&json!({"open_links": true})).is_err());
    let kept: Settings = serde_json::from_str(r#"{"theme": "dark"}"#).unwrap();
    assert_eq!(kept.open_links, OpenLinks::Ask, "a file from before asks");
}
