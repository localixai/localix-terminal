use tao::dpi::LogicalSize;

use super::{Frame, PRESETS, Picked, menu, parse, picked, place, script};

fn frame(name: &str, width: f64, height: f64) -> Frame {
    Frame {
        name: name.into(),
        width,
        height,
    }
}

#[test]
fn an_agent_names_a_size_as_a_person_would() {
    assert_eq!(
        parse("iPhone 15 Pro"),
        Ok(Some(frame("iPhone 15 Pro", 393.0, 852.0)))
    );
    assert_eq!(
        parse("iphone15pro"),
        Ok(Some(frame("iPhone 15 Pro", 393.0, 852.0)))
    );
    assert_eq!(
        parse("mobile"),
        Ok(Some(frame("iPhone 15 Pro", 393.0, 852.0)))
    );
    assert_eq!(parse("tablet"), Ok(Some(frame("iPad Mini", 768.0, 1024.0))));
    assert_eq!(parse("Desktop"), Ok(Some(frame("Desktop", 1440.0, 900.0))));
    assert_eq!(parse("390x844"), Ok(Some(frame("Custom", 390.0, 844.0))));
    assert_eq!(parse("390 × 844"), Ok(Some(frame("Custom", 390.0, 844.0))));
    assert_eq!(parse("window"), Ok(None));
    assert_eq!(parse(" Responsive "), Ok(None));
}

#[test]
fn a_size_it_does_not_know_says_what_it_takes() {
    for wrong in ["watch", "10x10", "390x", "x844", "99999x800", ""] {
        let said = parse(wrong).expect_err(wrong);
        assert!(
            said.contains("iPhone SE") && said.contains("WIDTHxHEIGHT"),
            "{said}"
        );
    }
}

#[test]
fn a_frame_sits_in_the_middle_and_never_past_the_window() {
    let part = LogicalSize::new(1200.0, 700.0);
    assert_eq!(place(part, 84.0, None), (0.0, 84.0, part));
    let phone = frame("iPhone SE", 375.0, 667.0);
    assert_eq!(
        place(part, 84.0, Some(&phone)),
        (412.0, 100.0, LogicalSize::new(375.0, 667.0))
    );
    // Taller than the window: cut to it, at its top.
    let tall = frame("iPhone 15 Pro", 393.0, 852.0);
    assert_eq!(
        place(part, 84.0, Some(&tall)),
        (403.0, 84.0, LogicalSize::new(393.0, 700.0))
    );
}

#[test]
fn the_menu_marks_the_size_set_and_turns_it() {
    let (items, marked) = menu(None);
    assert_eq!(
        items.len(),
        1 + PRESETS.len(),
        "no Rotate without a size: {items:?}"
    );
    assert_eq!(items[0], "Fit to Window");
    assert_eq!(items[2], "iPhone 15 Pro  393 × 852");
    assert_eq!(marked, Some(0));
    assert_eq!(
        picked(2, None),
        Some(Picked::Size(frame("iPhone 15 Pro", 393.0, 852.0)))
    );

    let now = frame("iPhone 15 Pro", 393.0, 852.0);
    let (items, marked) = menu(Some(&now));
    assert_eq!(marked, Some(2));
    assert_eq!(items.last().map(String::as_str), Some("Rotate"));
    let turned = picked(items.len() - 1, Some(&now));
    assert_eq!(
        turned,
        Some(Picked::Size(frame("iPhone 15 Pro", 852.0, 393.0)))
    );
    // Turned, it is still the one marked, and picking it again keeps it turned.
    let turned = frame("iPhone 15 Pro", 852.0, 393.0);
    assert_eq!(menu(Some(&turned)).1, Some(2));
    assert_eq!(picked(2, Some(&turned)), Some(Picked::Size(turned.clone())));
    assert_eq!(picked(0, Some(&turned)), Some(Picked::Window));
    assert_eq!(picked(99, Some(&turned)), None);

    // A size of the agent's own is listed while it is set.
    let own = frame("Custom", 390.0, 844.0);
    let (items, marked) = menu(Some(&own));
    assert_eq!(
        marked.map(|at| items[at].as_str()),
        Some("Custom  390 × 844")
    );
}

#[test]
fn the_strip_hears_the_size_as_data() {
    assert_eq!(script(None), "F(null)");
    let said = script(Some(&frame("</script>", 393.0, 852.0)));
    assert!(
        said.starts_with("F({") && said.contains(r#""width":393.0"#),
        "{said}"
    );
    assert!(said.contains(r#""name":"</script>""#), "{said}");
}
