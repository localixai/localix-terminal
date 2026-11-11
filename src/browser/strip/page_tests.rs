//! The strip's page as it is built: its one script run by its hash, the agents' marks in it as
//! data, its empty places, its bars as tall as the window lays them out; and the side's
//! width held as the layout holds it.

use sha2::Digest as _;

use super::*;

#[test]
fn the_assembled_page_contains_its_resize_helper_and_no_external_script() {
    let page = page("{}");
    assert!(page.contains("function resizeHandle("));
    assert!(!page.contains("/*RESIZE_HANDLES*/"));
    assert!(!page.contains("<script src="));
}

#[test]
fn the_page_runs_its_one_script_by_its_hash() {
    let page = page("{}");
    let script = page
        .split("<script>")
        .nth(1)
        .and_then(|s| s.split("</script>").next());
    let digest = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        sha2::Sha384::digest(script.unwrap_or_default()),
    );
    assert_eq!(
        SCRIPT_SHA384, digest,
        "strip.html's script changed: put its new SHA-384 here"
    );
    assert_eq!(page.matches("<script>").count(), 1, "one script runs");
    assert!(page.contains(&format!("script-src 'sha384-{SCRIPT_SHA384}'\"")));
    let policy = page
        .split("script-src")
        .nth(1)
        .and_then(|s| s.split('"').next());
    assert!(!policy.unwrap_or_default().contains("unsafe"), "{policy:?}");
    assert!(!page.contains("{{"), "every place is filled");
}

#[test]
fn the_agents_marks_reach_the_page_as_data_that_never_ends_its_element() {
    let marks = r#"{"x":"</script><script>alert(1)</script>"}"#;
    let page = page(marks);
    // JSON's own escape of `<`, which the script's parse reads back as it was.
    let escaped = marks.replace('<', concat!("\\", "u003c"));
    assert!(page.contains(&format!("id=\"marks\">{escaped}</script>")));
    assert_eq!(page.matches("<script>").count(), 1);
}

/// The height a rule of the built page gives, in its own words (`38px`).
fn rule_height(page: &str, selector: &str) -> Option<String> {
    let rule = page.split(&format!("\n{selector}{{")).nth(1)?;
    let rule = rule.split('}').next()?;
    let height = rule.split(";height:").nth(1)?;
    height.split(';').next().map(str::to_string)
}

#[test]
fn the_page_draws_its_bars_as_tall_as_the_window_lays_them_out() {
    let page = page("{}");
    // The bar is a compact toolbar's height, the browser's toolbar `NAV`'s, the tabs' row the
    // rest of the strip.
    let px = |points: f64| Some(format!("{points}px"));
    assert_eq!(rule_height(&page, "#bar"), px(BAR));
    assert_eq!(rule_height(&page, "html[data-part=nav] #browse"), px(NAV));
    assert_eq!(rule_height(&page, "#tabs"), px(HEIGHT - BAR));
    assert_eq!((BAR, NAV, HEIGHT), (38.0, 40.0, 70.0));
}

#[test]
fn the_side_is_held_between_its_narrowest_and_widest_and_to_half_the_window() {
    // A width kept from another window: only its own bounds.
    assert_eq!(side_held(100.0, None), SIDE_MIN);
    assert_eq!(side_held(300.0, None), 300.0);
    assert_eq!(side_held(900.0, None), SIDE_MAX);
    // Dragged in a window: never more than half of it, never under the narrowest.
    assert_eq!(side_held(450.0, Some(800.0)), 400.0);
    assert_eq!(side_held(450.0, Some(1200.0)), 450.0);
    assert_eq!(side_held(300.0, Some(300.0)), SIDE_MIN);
    assert_eq!(side_widest(Some(300.0)), SIDE_MIN);
    assert_eq!(side_widest(Some(700.0)), 350.0);
    assert_eq!(side_widest(None), SIDE_MAX);
}

#[test]
fn an_empty_place_is_one_quiet_line_and_an_empty_window_its_first_commands() {
    // Each empty place says what would be there, in one line: no glyph, no button.
    for line in ["No Tabs", "No Matching Tabs", "No History", "No Events"] {
        assert!(PAGE.contains(&format!("emptied(\"{line}\")")), "{line}");
    }
    assert!(!PAGE.contains("function doing(") && !PAGE.contains(".empty .g"));
    // The window with no tab left offers what opens one, each with its key as Settings sets it.
    let start = (PAGE.split("const STARTS = [").nth(1))
        .and_then(|rest| rest.split("];").next())
        .unwrap_or_default();
    for (id, words) in [
        ("new-page", "key new page"),
        ("new-terminal", "key new terminal"),
        ("board", "key new board"),
        ("open-folder", "key open folder"),
    ] {
        assert!(
            start.contains(&format!("[\"{id}\"")) && start.contains(words),
            "{id}"
        );
    }
    // No picture and no motion of its own: the page draws nothing it does not say.
    assert!(!PAGE.contains("img-src") && !PAGE.contains("@keyframes wind"));
}
