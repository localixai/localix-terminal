//! Box characters as lines that reach their cell's edges, so they join; blocks fill their part of
//! the cell; diagonals and other characters are the font's.

use super::{Piece, drawn, pieces};

fn reaches(pieces: &[Piece], edge: &str, width: f64, height: f64) -> bool {
    pieces.iter().any(|p| match edge {
        "left" => p.x == 0.0,
        "right" => p.x + p.width == width,
        "top" => p.y == 0.0,
        "bottom" => p.y + p.height == height,
        _ => false,
    })
}

#[test]
fn a_line_reaches_the_edges_it_points_to() {
    let (w, h) = (16.0, 34.0);
    let horizontal = pieces('─', w, h, 2.0);
    assert!(reaches(&horizontal, "left", w, h) && reaches(&horizontal, "right", w, h));
    assert!(!reaches(&horizontal, "top", w, h));
    let corner = pieces('┌', w, h, 2.0);
    assert!(reaches(&corner, "right", w, h) && reaches(&corner, "bottom", w, h));
    assert!(!reaches(&corner, "left", w, h) && !reaches(&corner, "top", w, h));
    let cross = pieces('╋', w, h, 2.0);
    assert!(
        cross.iter().all(|p| p.width == 4.0 || p.height == 4.0),
        "heavy is twice as thick"
    );
    assert_eq!(
        pieces('═', w, h, 2.0).len(),
        4,
        "double is two lines each side"
    );
}

#[test]
fn the_arms_of_a_corner_meet() {
    let corner = pieces('┐', 16.0, 34.0, 2.0);
    let (across, down) = (corner[0], corner[1]);
    assert!(
        across.x + across.width >= down.x + down.width,
        "the horizontal arm covers the joint"
    );
    assert!(down.y <= across.y, "the vertical arm starts above it");
}

#[test]
fn blocks_fill_their_part_of_the_cell() {
    assert_eq!(
        pieces('█', 16.0, 32.0, 2.0),
        [Piece {
            x: 0.0,
            y: 0.0,
            width: 16.0,
            height: 32.0,
            share: 1.0
        }]
    );
    assert_eq!(pieces('▄', 16.0, 32.0, 2.0)[0].y, 16.0);
    assert_eq!(pieces('▌', 16.0, 32.0, 2.0)[0].width, 8.0);
    assert_eq!(pieces('▒', 16.0, 32.0, 2.0)[0].share, 0.5);
    assert_eq!(pieces('▚', 16.0, 32.0, 2.0).len(), 2);
}

#[test]
fn only_box_characters_are_drawn_here() {
    assert!(drawn('│') && drawn('╭') && drawn('▀'));
    assert!(!drawn('╱') && !drawn('a') && !drawn('■'));
}
