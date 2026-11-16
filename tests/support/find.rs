//! Find in a terminal's output in the real page, driven as the person drives it: the window's
//! Find, Find Next and Find Previous (the clicks the Edit menu's items fire), what is typed into
//! the find row, Return and Shift+Return, the count it says over the scrollback, a full-screen
//! program's screen, and Esc, which gives the keys back to the terminal. Nothing typed in the row
//! reaches the program.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use llxterm::browser::{Browser, Panel};
use serde_json::{Value, json};

/// What the program wrote, run in the page as the session paints it.
fn write(bytes: &str) -> String {
    format!(
        "W(\"{}\")",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// A key pressed in the find row's field.
fn key(init: &Value) -> String {
    format!(
        "(() => {{ const f = document.getElementById('find-q'); \
         f.dispatchEvent(new KeyboardEvent('keydown', \
         Object.assign({{bubbles: true, cancelable: true}}, {init}))); return 1 }})()"
    )
}

/// What the row says, where the keys are and the grid's height.
const READ: &str = "(() => ({ open: !document.getElementById('find').hidden, \
    count: document.getElementById('find-n').textContent, \
    keys: document.activeElement === document.getElementById('find-q') ? 'field' \
      : document.activeElement === term.textarea ? 'terminal' : String(document.activeElement?.id), \
    rows: term.rows }))()";

/// How many times the buffer shown holds the query, in any case: what the count must say.
const HELD: &str = "(() => { const b = term.buffer.active; let n = 0; \
    for (let i = 0; i < b.length; i++) \
      n += b.getLine(i).translateToString(true).toLowerCase().split('needle').length - 1; \
    return b.type === 'normal' ? n : -1 })()";

pub(crate) fn check(browser: &Browser, panel: &Arc<Panel>) {
    let typed = Arc::new(Mutex::new(String::new()));
    let into = Arc::clone(&typed);
    panel.keyboard(Arc::new(move |keys: &str| {
        into.lock().unwrap().push_str(keys)
    }));
    let run = |script: &str| browser.peek(panel, script).unwrap();
    // The row once `done` holds of it, within a while.
    let until = |done: &dyn Fn(&Value) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let read = run(READ);
            if done(&read) {
                return read;
            }
            assert!(Instant::now() < deadline, "the find row shows {read:#}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let counted = |want: &'static str| {
        until(&move |read: &Value| read["count"] == want);
    };

    // A hundred lines, more than the screen holds: the first match is in the scrollback, and
    // each is in another case than the query.
    let lines: Vec<String> = (0..100)
        .map(|n| match n {
            2 => "a needle in the scrollback".to_string(),
            50 => "NEEDLE in the middle".to_string(),
            98 => "the last Needle".to_string(),
            n => format!("line {n}"),
        })
        .collect();
    run(&write(&format!(
        "\x1b[3J\x1b[H\x1b[2J{}",
        lines.join("\r\n")
    )));
    let before = until(&|read: &Value| read["open"] == false)["rows"].clone();

    // Find opens the row with the keys in its field; the screen ends over it.
    browser.click("key find text");
    until(&|read: &Value| read["open"] == true && read["keys"] == "field");
    until(&|read: &Value| read["rows"].as_u64() < before.as_u64());
    run(
        "(() => { const f = document.getElementById('find-q'); f.value = 'needle'; \
         f.dispatchEvent(new Event('input')); return 1 })()",
    );
    counted("1 of 3");

    // The window's Find Next and Find Previous, and Return and Shift+Return in the field.
    browser.click("key find next");
    counted("2 of 3");
    browser.click("key find previous");
    counted("1 of 3");
    run(&key(&json!({"key": "Enter"})));
    counted("2 of 3");
    run(&key(&json!({"key": "Enter", "shiftKey": true})));
    counted("1 of 3");
    run(
        "(() => { const f = document.getElementById('find-q'); f.value = 'haystack'; \
         f.dispatchEvent(new Event('input')); return 1 })()",
    );
    counted("No results");
    run(
        "(() => { const f = document.getElementById('find-q'); f.value = 'needle'; \
         f.dispatchEvent(new Event('input')); return 1 })()",
    );
    counted("1 of 3");

    // What the program prints next is searched as it comes.
    run(&write("\r\none more needle"));
    until(&|read: &Value| read["count"].as_str().is_some_and(|c| c.ends_with(" of 4")));

    // A full-screen program: its screen alone, which has no scrollback; the shell's after it,
    // as many as its buffer holds (the grid is fitted anew on each switch, and a shell draws
    // its last line again then).
    run(&write(
        "\x1b[?1049h\x1b[H\x1b[2Jthe needle on a full screen",
    ));
    counted("1 of 1");
    run(&write("\x1b[?1049l"));
    let held = |read: &Value| {
        let held = run(HELD).as_u64().unwrap_or(0);
        held >= 3
            && read["count"]
                .as_str()
                .is_some_and(|c| c.ends_with(&format!(" of {held}")))
    };
    until(&held);

    // Esc closes the row and gives the keys back to the terminal, at its height again.
    run(&key(&json!({"key": "Escape"})));
    let read = until(&|read: &Value| read["open"] == false && read["rows"] == before);
    assert_eq!(read["keys"], "terminal", "{read}");
    // Find Next with the row shut opens it on what it held.
    browser.click("key find next");
    until(&|read: &Value| read["open"] == true && read["keys"] == "field");
    run(&key(&json!({"key": "Escape"})));
    until(&|read: &Value| read["open"] == false && read["rows"] == before);

    // Nothing typed into the row, nor its keys, went to the program.
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        typed.lock().unwrap().as_str(),
        "",
        "the find row typed into the session"
    );
    // The terminal as the scenario had it: no scrollback, an empty screen.
    run(&write("\x1b[3J\x1b[H\x1b[2J"));
    let empty = "term.buffer.active.length === term.rows";
    let deadline = Instant::now() + Duration::from_secs(5);
    while run(empty) != json!(true) {
        assert!(Instant::now() < deadline, "the scrollback stays");
        std::thread::sleep(Duration::from_millis(20));
    }
}
