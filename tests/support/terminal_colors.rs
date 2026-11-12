//! A color-aware program's PTY output and full-screen replay, read from the real xterm panel.

use std::sync::Arc;
use std::time::{Duration, Instant};

use llxterm::browser::{Browser, Panel};
use llxterm::terminal::{DEFAULT_WAIT, Session, Until};
use serde_json::json;

const CELLS: &str = r#"(() => {
    const line = term.buffer.active.getLine(0);
    return [0, 1, 2].map(x => {
        const cell = line && line.getCell(x);
        return cell ? {
            text: cell.getChars(), color: cell.getFgColor(),
            palette: cell.isFgPalette(), rgb: cell.isFgRGB()
        } : null;
    });
})()"#;

pub(crate) fn check(browser: &Browser, panel: &Arc<Panel>) {
    let vars = [
        ("PATH", "/usr/bin:/bin"),
        ("TERM", "dumb"),
        ("NO_COLOR", "1"),
        ("FORCE_COLOR", "0"),
        ("CLICOLOR", "0"),
    ]
    .map(|(name, value)| (name.to_string(), value.to_string()));
    let draw = Arc::clone(panel);
    let session = Session::start(
        r#"printf '\033[H\033[2J'
        if [ -t 1 ] && [ "${NO_COLOR+x}" != x ] && [ "$CLICOLOR" = 1 ]; then
            printf '\033[31mR\033[38;5;202mI\033[38;2;12;180;240mT\033[0m'
        else
            printf RIT
        fi
        printf '\r\nCOLOR-READY\r\n'; sleep 20"#,
        &std::env::temp_dir(),
        Some(&vars),
        Some(Box::new(move |script| draw.paint(&script))),
    )
    .unwrap();
    assert_eq!(
        session.wait(Some("COLOR-READY"), 0, DEFAULT_WAIT),
        Until::Matched
    );
    let expected = json!([
        {"text": "R", "color": 1, "palette": true, "rgb": false},
        {"text": "I", "color": 202, "palette": true, "rgb": false},
        {"text": "T", "color": 0x0cb4f0, "palette": false, "rgb": true}
    ]);
    let colored = || {
        let deadline = Instant::now() + DEFAULT_WAIT;
        loop {
            let read = browser.peek(panel, CELLS);
            if read.as_ref() == Ok(&expected) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the terminal colors are {read:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    colored();
    assert_eq!(
        browser
            .peek(
                panel,
                "term.options.theme.red !== term.options.theme.foreground"
            )
            .unwrap(),
        json!(true),
    );
    // A new page starts blank: the session restores all three color modes, not plain text.
    panel.paint("R('')");
    let deadline = Instant::now() + DEFAULT_WAIT;
    while browser.peek(
        panel,
        "term.buffer.active.getLine(0).translateToString(true)",
    ) != Ok(json!(""))
    {
        assert!(Instant::now() < deadline, "the terminal did not reset");
        std::thread::sleep(Duration::from_millis(20));
    }
    session.behind();
    colored();
}
