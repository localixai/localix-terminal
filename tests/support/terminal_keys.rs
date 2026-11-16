//! The person's keys in the real terminal page (xterm's own encoding behind the page's): what
//! Shift+Enter and Option send, what ⌘K leaves of a full-screen program, and a program's copy
//! (OSC 52): posted by the page to the window, which keeps it off the pasteboard while the
//! terminal does not have the person's keys.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use llxterm::browser::{Browser, Panel};
use serde_json::json;

/// A key pressed in the page, as WebKit hands it to xterm's textarea: `init` of a KeyboardEvent,
/// and the legacy key code xterm's encoding reads (a constructed event has none of its own).
fn key(init: &serde_json::Value, code: u32) -> String {
    format!(
        "(() => {{ const e = new KeyboardEvent({init}.type || 'keydown', \
         Object.assign({{bubbles: true, cancelable: true}}, {init})); \
         Object.defineProperty(e, 'keyCode', {{get: () => {code}}}); \
         term.textarea.dispatchEvent(e); return 1 }})()"
    )
}

/// What the program wrote, run in the page as the session paints it.
fn write(bytes: &str) -> String {
    format!(
        "W(\"{}\")",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

pub(crate) fn check(browser: &Browser, panel: &Arc<Panel>) {
    let typed = Arc::new(Mutex::new(String::new()));
    let into = Arc::clone(&typed);
    panel.keyboard(Arc::new(move |keys: &str| {
        into.lock().unwrap().push_str(keys)
    }));
    let run = |script: &str| browser.peek(panel, script).unwrap();
    // What the page typed once `want` came, within a while; nothing else may come before it.
    let until = |want: &str| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let now = typed.lock().unwrap().clone();
            if now.len() >= want.len() || Instant::now() > deadline {
                typed.lock().unwrap().clear();
                return now;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    };

    // Shift+Enter is LF, a newline for a coding agent's prompt; Enter alone the end of the line.
    run(&key(
        &json!({"key": "Enter", "code": "Enter", "shiftKey": true}),
        13,
    ));
    assert_eq!(until("\n"), "\n");
    run(&key(&json!({"key": "Enter", "code": "Enter"}), 13));
    assert_eq!(until("\r"), "\r");

    // The left Option made Meta, as Settings does: Option+B is ESC b (a word back in readline).
    // The right one types characters, which the system's input gives the page; a constructed
    // key has none to give.
    let look = |option: &str| {
        format!(
            "dispatchEvent(new CustomEvent('desk-look', {{detail: \
             Object.assign({{}}, window.__deskLook, {{option: '{option}'}})}})); 1"
        )
    };
    run(&look("left"));
    let option = |side: &str, kind: &str| {
        key(
            &json!({"type": kind, "key": "Alt", "code": side, "altKey": kind == "keydown"}),
            18,
        )
    };
    let b = key(&json!({"key": "∫", "code": "KeyB", "altKey": true}), 66);
    run(&option("AltLeft", "keydown"));
    run(&b);
    run(&option("AltLeft", "keyup"));
    assert_eq!(until("\x1bb"), "\x1bb");
    run(&option("AltRight", "keydown"));
    run(&b);
    run(&key(
        &json!({"key": "Enter", "code": "Enter", "altKey": true}),
        13,
    ));
    run(&option("AltRight", "keyup"));
    assert_eq!(
        until("\r"),
        "\r",
        "Enter from an Option that types, and nothing for ∫"
    );
    run(&option("AltLeft", "keydown"));
    run(&key(
        &json!({"key": "Enter", "code": "Enter", "altKey": true}),
        13,
    ));
    run(&option("AltLeft", "keyup"));
    assert_eq!(
        until("\x1b\r"),
        "\x1b\r",
        "Meta Return from the Meta Option"
    );
    run(&look("off"));

    // ⌘K leaves a full-screen program's screen as it drew it, and types nothing into it.
    let first_row =
        "term.buffer.active.getLine(term.buffer.active.viewportY)?.translateToString(true)";
    run(&write("\x1b[?1049h\x1b[HFULL SCREEN"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while run(first_row) != json!("FULL SCREEN") {
        assert!(Instant::now() < deadline, "{}", run(first_row));
        std::thread::sleep(Duration::from_millis(20));
    }
    run(&key(
        &json!({"key": "k", "code": "KeyK", "metaKey": true}),
        75,
    ));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(run(first_row), json!("FULL SCREEN"));
    run(&write("\x1b[?1049l"));
    run(&key(&json!({"key": "Enter", "code": "Enter"}), 13));
    assert_eq!(until("\r"), "\r", "no form feed went to the program");

    #[cfg(target_os = "macos")]
    copies_only_with_the_keys(&run);
}

/// A program's copy (OSC 52) as a test run can see it, with the app never made active (an agent
/// runs this from a shell in the background, while the person uses the screen): the page posts
/// the copy to the window (`c` and its base64), and the window, which is not the key one, puts
/// none of it on the pasteboard. Where the keys must be for a copy to go is the window's rule
/// (`panel::copy_where`), checked by its unit test with the keys given. The check never writes
/// the person's pasteboard; were the window to, what it held is put back, whatever happens.
#[cfg(target_os = "macos")]
fn copies_only_with_the_keys(run: &dyn Fn(&str) -> serde_json::Value) {
    use super::pasteboard::{Saved, pasted};
    let encode = |text: &str| base64::engine::general_purpose::STANDARD.encode(text);
    // Text no one but this check copies: seen on the pasteboard, the window wrote it.
    let probe = format!("llxterm display test {}", std::process::id());
    let _kept = Saved::now(&probe);
    assert!(
        !super::panel::active(),
        "a test run is never the app in front"
    );
    // What the page posts to the window, seen on its way there: it still goes.
    let watched = run("(() => { const ipc = window.webkit.messageHandlers.ipc; \
         if (!window.__copies) { window.__copies = []; const send = ipc.postMessage.bind(ipc); \
         ipc.postMessage = (said) => { if (String(said).startsWith('c ')) \
         window.__copies.push(String(said)); return send(said) } } \
         return window.webkit.messageHandlers.ipc.postMessage !== undefined && \
         Array.isArray(window.__copies) })()");
    assert_eq!(watched, json!(true), "the page's posts can be watched");
    run(&write(&format!("\x1b]52;c;{}\x07", encode(&probe))));
    // A program asking to read the pasteboard is never answered, and posts nothing.
    run(&write("\x1b]52;c;?\x07"));
    let deadline = Instant::now() + Duration::from_secs(5);
    let posted = loop {
        let posted = run("window.__copies");
        if posted != json!([]) || Instant::now() > deadline {
            break posted;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(
        posted,
        json!([format!("c {}", encode(&probe))]),
        "the page posts the copy"
    );
    // The window had it: a window that is not the key one puts none of it on the pasteboard.
    std::thread::sleep(Duration::from_millis(300));
    assert_ne!(pasted(), probe, "a terminal without the keys copied");
}
