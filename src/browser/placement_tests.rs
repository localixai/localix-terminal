//! Tests of `placement`.

use super::*;

const LAPTOP: Area = Area {
    x: 0.0,
    y: 38.0,
    width: 1512.0,
    height: 870.0,
};

/// A second screen to the right of the laptop's, larger and set higher.
const DESK: Area = Area {
    x: 1512.0,
    y: -300.0,
    width: 2560.0,
    height: 1400.0,
};

#[test]
fn the_window_opens_in_the_middle_of_the_screen_and_leaves_what_is_around_it_in_sight() {
    // A laptop's screen is not filled: about three quarters of it each way.
    let at = deck(&[LAPTOP], None);
    assert_eq!((at.width, at.height), (1149.0, 696.0));
    assert_eq!((at.x, at.y), (182.0, LAPTOP.y + 58.0));
    // A large screen gets no more than reads well, a third of what is left above it, as the
    // system centres a window.
    let big = Area {
        width: 3008.0,
        height: 1600.0,
        ..LAPTOP
    };
    let at = deck(&[big], None);
    assert_eq!((at.width, at.height), (1200.0, 760.0));
    assert_eq!((at.x, at.y), (904.0, 38.0 + 280.0));
    // Never wider than the screen itself.
    let tiny = Area {
        width: 800.0,
        ..LAPTOP
    };
    let at = deck(&[tiny], None);
    assert_eq!((at.x, at.width), (MARGIN, 800.0 - 2.0 * MARGIN));
}

#[test]
fn a_frame_the_person_gave_it_is_kept_on_whichever_screen_it_lies() {
    let kept = Rect {
        x: 100.0,
        y: 120.0,
        width: 700.0,
        height: 500.0,
    };
    assert_eq!(deck(&[LAPTOP], Some(kept)), kept);
    let beside = Rect {
        x: 2000.0,
        y: -100.0,
        width: 1600.0,
        height: 1000.0,
    };
    assert_eq!(deck(&[LAPTOP, DESK], Some(beside)), beside);
    assert_eq!(home(&[LAPTOP, DESK], beside), DESK);
    // Hanging off an edge as the person left it, it stays: enough of it is in reach.
    let hanging = Rect { x: -300.0, ..kept };
    assert_eq!(deck(&[LAPTOP], Some(hanging)), hanging);
}

#[test]
fn a_frame_kept_for_a_screen_that_is_gone_is_not_opened_out_of_reach() {
    let beside = Rect {
        x: 2000.0,
        y: -100.0,
        width: 1600.0,
        height: 1000.0,
    };
    // The second screen was unplugged: the window opens as a first one does.
    assert_eq!(deck(&[LAPTOP], Some(beside)), deck(&[LAPTOP], None));
    // A sliver left on the screen is out of reach as well.
    let sliver = Rect {
        x: LAPTOP.width - 40.0,
        ..beside
    };
    assert_eq!(deck(&[LAPTOP], Some(sliver)), deck(&[LAPTOP], None));
    // Larger than the screen it is on now (a smaller one took its place), it is held to it.
    let large = Rect {
        x: 0.0,
        y: 38.0,
        width: 2400.0,
        height: 1300.0,
    };
    let at = deck(&[LAPTOP], Some(large));
    assert_eq!((at.width, at.height), (LAPTOP.width, LAPTOP.height));
    assert_eq!((at.x, at.y), (LAPTOP.x, LAPTOP.y));
    // And never smaller than a window is made.
    let small = Rect {
        width: 10.0,
        height: 10.0,
        ..large
    };
    let at = deck(&[LAPTOP], Some(small));
    assert_eq!((at.width, at.height), (LEAST.width, LEAST.height));
}

#[test]
fn what_the_person_set_is_kept_in_a_file_between_launches() {
    let dir = std::env::temp_dir().join(format!("llx-desk-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("state").join("window.json");
    // Nothing kept yet, or a file that does not read: as on a first launch.
    assert!(Kept::read(&file).deck.is_none());
    let kept = Kept {
        deck: Some(Rect {
            x: 100.0,
            y: 120.0,
            width: 700.0,
            height: 500.0,
        }),
        side: Some(false),
        side_width: Some(260.0),
    };
    kept.write(&file).unwrap();
    let read = Kept::read(&file);
    assert_eq!(
        (read.deck, read.side, read.side_width),
        (kept.deck, Some(false), Some(260.0))
    );
    std::fs::write(&file, "{ not json").unwrap();
    let read = Kept::read(&file);
    assert!(read.deck.is_none() && read.side.is_none());
    // A frame that is no frame (a hand's edit) is none.
    std::fs::write(
        &file,
        r#"{"deck": {"x": 0, "y": 0, "width": -5, "height": 1e999}}"#,
    )
    .unwrap();
    assert!(Kept::read(&file).deck.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_window_after_the_main_one_has_a_place_of_its_own_inside_the_screen() {
    let main = Rect {
        x: 100.0,
        y: 80.0,
        width: 800.0,
        height: 500.0,
    };
    let places: Vec<Rect> = (1..=3).map(|steps| cascade(LAPTOP, main, steps)).collect();
    assert_eq!((places[0].x, places[0].y), (128.0, 108.0));
    assert_eq!((places[1].x, places[1].y), (156.0, 136.0));
    assert_ne!((places[1].x, places[1].y), (places[2].x, places[2].y));
    // A window that fills the screen's height still moves, and stays on the screen.
    let tall = Rect {
        x: 12.0,
        y: 50.0,
        width: 1400.0,
        height: 846.0,
    };
    let (one, two) = (cascade(LAPTOP, tall, 1), cascade(LAPTOP, tall, 2));
    assert_ne!((one.x, one.y), (two.x, two.y));
    for at in [one, two] {
        assert!(at.x >= LAPTOP.x && at.x + at.width <= LAPTOP.x + LAPTOP.width);
        assert!(at.y >= LAPTOP.y && at.y + at.height <= LAPTOP.y + LAPTOP.height);
    }
}
