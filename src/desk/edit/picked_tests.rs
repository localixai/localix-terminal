//! Tests of how a file the person picked opens: text in an editor, a picture written as text
//! and what no editor takes as a page, and nothing where there is no file.

use std::path::Path;

use crate::desk::Call;
use crate::desk::edit_disk::not_opened;
use crate::desk::tests::{caller, desk};
use crate::files::text::Unread;
use crate::testing::Scratch;

#[test]
fn a_picked_file_an_editor_does_not_take_is_a_pages_to_show() {
    // Whatever is there and is no text an editor takes: a picture, a film, an archive, a
    // program, a log too long. A page shows it, or says in a line that nothing does.
    for name in ["logo.PNG", "clip.mp4", "build.zip", "tool"] {
        let path = Path::new("/work/app").join(name);
        assert!(not_opened(&path, &Unread::Binary).page, "{name}");
    }
    let log = Path::new("/work/app/all.log");
    assert!(not_opened(log, &Unread::Large(64 << 20)).page);
    // What is not there, or is no file, opens nowhere.
    let none = Path::new("/work/app/none.rs");
    assert!(!not_opened(none, &Unread::Gone).page);
    assert!(!not_opened(Path::new("/work/app"), &Unread::Folder).page);
}

#[test]
fn a_picture_written_as_text_is_looked_at_and_its_text_is_the_second_way() {
    let scratch = Scratch::new();
    scratch.file(
        "art/mark.svg",
        "<svg xmlns=\"http://www.w3.org/2000/svg\"/>\n",
    );
    scratch.file("site/index.html", "<p>hi</p>\n");
    let desk = desk();
    let me = caller(&desk, scratch.path(), None);
    let person = Call {
        person: true,
        ..me.call()
    };
    let edited = || crate::lock(&desk.edits).len();
    // Picked, it is a page's to draw (which no window is there to open here), never an
    // editor's tab.
    desk.person_file(&person, &scratch.path().join("art/mark.svg"))
        .unwrap();
    assert_eq!(edited(), 0);
    // Asked for as text, it is edited as any text.
    desk.person_text(&person, &scratch.path().join("art/mark.svg"))
        .unwrap();
    assert_eq!(edited(), 1);
    // Markup a page renders is written far more than looked at: an editor first.
    desk.person_file(&person, &scratch.path().join("site/index.html"))
        .unwrap();
    assert_eq!(edited(), 2);
}
