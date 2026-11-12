//! Tests of `files::text`: a file read as an editor holds it, written back as the disk had it,
//! what is never read as text, and a change on disk told from the editor's own save.

use std::os::unix::fs::PermissionsExt as _;

use super::{LARGEST, Shape, Unread, news, read, save};
use crate::testing::Scratch;

#[test]
fn a_file_is_read_with_plain_line_ends_and_written_back_as_the_disk_had_it() {
    let dir = Scratch::new();
    let path = dir.file("a.txt", "\u{feff}one\r\ntwo\r\n");
    let held = read(&path).unwrap();
    assert_eq!(held.text, "one\ntwo\n");
    assert_eq!(
        held.shape,
        Shape {
            crlf: true,
            bom: true
        }
    );
    save(&path, "one\ntwo\nthree\n", held.shape).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"\xef\xbb\xbfone\r\ntwo\r\nthree\r\n"
    );
}

#[test]
fn a_plain_file_keeps_its_line_ends_and_its_missing_last_one() {
    let dir = Scratch::new();
    let path = dir.file("a.rs", "fn main() {}\n// é\nlast");
    let held = read(&path).unwrap();
    assert_eq!(held.text, "fn main() {}\n// é\nlast");
    assert_eq!(held.shape, Shape::default());
    save(&path, &held.text, held.shape).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fn main() {}\n// é\nlast"
    );
}

#[test]
fn what_is_not_text_is_not_read_as_text() {
    let dir = Scratch::new();
    assert!(matches!(read(&dir.path().join("none")), Err(Unread::Gone)));
    assert!(matches!(read(dir.path()), Err(Unread::Folder)));
    let zeroes = dir.file("a.bin", b"\x7fELF\0\0\0");
    assert!(matches!(read(&zeroes), Err(Unread::Binary)));
    let latin = dir.file("latin.txt", b"caf\xe9\n");
    assert!(matches!(read(&latin), Err(Unread::Binary)));
    let large = dir.file("large.txt", "");
    let file = std::fs::File::options().write(true).open(&large).unwrap();
    file.set_len(LARGEST + 1).unwrap();
    assert!(matches!(read(&large), Err(Unread::Large(len)) if len == LARGEST + 1));
}

#[test]
fn a_save_writes_the_file_in_place() {
    let dir = Scratch::new();
    let real = dir.file("real.sh", "old\n");
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o750)).unwrap();
    let link = dir.path().join("link.sh");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let held = read(&link).unwrap();
    save(&link, "new\n", held.shape).unwrap();
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "new\n");
    assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
    let mode = std::fs::metadata(&real).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o750);
}

#[test]
fn a_save_makes_a_file_that_is_gone() {
    let dir = Scratch::new();
    let path = dir.path().join("new.txt");
    save(&path, "made\n", Shape::default()).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "made\n");
}

#[test]
fn the_editors_own_save_is_no_news_and_anothers_write_is() {
    let dir = Scratch::new();
    let path = dir.file("a.txt", "one\n");
    let held = read(&path).unwrap();
    assert!(news(&path, &held.stamp).is_none());
    let stamp = save(&path, "two\n", held.shape).unwrap();
    assert!(news(&path, &stamp).is_none());
    // Written again with the same text: a later time, and nothing new to show.
    std::fs::write(&path, "two\n").unwrap();
    assert!(news(&path, &stamp).is_none());
    std::fs::write(&path, "three\n").unwrap();
    let told = news(&path, &stamp).unwrap().unwrap();
    assert_eq!(told.text, "three\n");
    assert!(news(&path, &told.stamp).is_none());
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(news(&path, &told.stamp), Some(Err(Unread::Gone))));
}
