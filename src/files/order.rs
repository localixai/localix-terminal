//! The order of names in a folder, as a file manager sorts them: letters without regard to
//! case, and a run of digits by the number it writes (`file2` before `file10`).

use std::cmp::Ordering;
use std::iter::Peekable;
use std::str::Chars;

/// `a` against `b`: by letters regardless of case and digits as numbers; names equal that way
/// are told apart as they are written, so the order is total.
pub(super) fn natural(a: &str, b: &str) -> Ordering {
    let (mut left, mut right) = (a.chars().peekable(), b.chars().peekable());
    loop {
        let found = match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                number(&mut left).cmp(&number(&mut right))
            }
            (Some(x), Some(y)) => {
                left.next();
                right.next();
                x.to_lowercase().cmp(y.to_lowercase())
            }
        };
        if found != Ordering::Equal {
            return found;
        }
    }
}

/// The run of digits `chars` is at, taken from it: how long it is without its leading zeros,
/// then its digits. Two of them compare as the numbers they write, however long.
fn number(chars: &mut Peekable<Chars<'_>>) -> (usize, String) {
    let mut digits = String::new();
    while let Some(digit) = chars.next_if(char::is_ascii_digit) {
        if digit != '0' || !digits.is_empty() {
            digits.push(digit);
        }
    }
    (digits.len(), digits)
}
