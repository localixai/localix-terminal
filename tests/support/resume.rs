//! A coding agent left running in a terminal goes on in its tab after the app opens again: a
//! stand-in for Claude Code (a script named as the agent of the app's table) is typed into the
//! person's shell, the app quits, and the app run again types the command that resumes the
//! same session into the same terminal's fresh shell, under the line that says so.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use llxterm::browser::Browser;
use serde_json::{Value, json};

const ID: &str = "0b8f6a52-6c1e-4b5e-9a57-3d5f0c2e9a11";

fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !want() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The stand-in: it says what it was started with and waits for a line under the agent's name.
fn stand_in(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(bin).unwrap();
    let script = "#!/bin/bash\nexec -a claude /bin/bash -c 'echo \"stand-in claude $*\"; sleep 1; \
                  echo ready; read -r line' stand-in \"$@\"\n";
    std::fs::write(bin.join("claude"), script).unwrap();
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o755)).unwrap();
}

pub(crate) fn scenario(browser: &Browser) {
    let home = std::env::temp_dir().join(format!("llxterm-resume-{}", std::process::id()));
    std::fs::create_dir_all(home.join(".git")).unwrap();
    let bin = home.join("bin");
    stand_in(&bin);
    let shell_home = home.join("home");
    std::fs::create_dir_all(&shell_home).unwrap();
    std::fs::write(shell_home.join(".zshrc"), "PS1='ready> '\n").unwrap();
    let vars = vec![
        ("SHELL".to_string(), "/bin/zsh".to_string()),
        ("HOME".to_string(), shell_home.display().to_string()),
        (
            "PATH".to_string(),
            format!("{}:/usr/bin:/bin", bin.display()),
        ),
    ];
    let store = Arc::new(llxterm::store::Store::memory().unwrap());
    let desk = |store: &Arc<llxterm::store::Store>| {
        let desk = llxterm::desk::Desk::new(
            Some(browser.clone()),
            true,
            home.join("shots"),
            Arc::new(llxterm::desk::Quiet),
            Arc::clone(store),
        );
        desk.keep_shell_files_in(home.join("shell"));
        desk
    };
    let first = desk(&store);
    let project = first.project(&home).unwrap();
    let owner = llxterm::projects::owner(&project, None);
    let call = llxterm::desk::Call {
        owner: &owner,
        project: &project,
        agent: None,
        cwd: &home,
        vars: Some(&vars),
        peek: false,
        again: None,
        person: true,
        window: None,
        from: None,
        runs: None,
    };
    let listed =
        |id: &str| (browser.listing()).is_some_and(|tabs| tabs.iter().any(|tab| tab.id == id));
    let opened = first
        .window(
            &call,
            &json!({"action": "open", "kind": "terminal", "wait_for": "ready>"}),
        )
        .unwrap();
    assert!(opened.contains("t1"), "{opened}");
    until("the terminal's tab is shown", || listed("t1"));
    let typed = json!({"action": "interact", "tab": "t1", "wait_for": "ready",
        "input": format!("claude --model opus --resume {ID} fix the cart\n")});
    let said = first.window(&call, &typed).unwrap();
    assert!(said.contains("\"ready\" showed"), "{said}");
    // The app quits with the agent at work: its terminal stays open in the ledger.
    std::thread::sleep(Duration::from_millis(300));
    first.shutdown();
    browser.click("close t1");
    until("the window is left with no tab", || !listed("t1"));

    // The app opens again: the same terminal, the person's shell, the agent's command typed.
    let again = desk(&store);
    again.project(&home).unwrap();
    again.reopen_for(&call).unwrap();
    until("the terminal is back in the window", || listed("t1"));
    let wanted = format!("stand-in claude --resume {ID} --model opus");
    let read: Value = json!({"action": "interact", "tab": "t1", "yield_ms": 200});
    let screen = || again.window(&call, &read).unwrap();
    until("the command that resumes is typed and runs", || {
        screen().contains(&wanted)
    });
    let screen = screen();
    assert!(
        screen.contains("── Resumed Claude Code session"),
        "{screen}"
    );
    // The prompt the agent had is not carried: it waits for the person.
    assert!(!screen.contains(&format!("{wanted} fix")), "{screen}");
    // Closed while the app runs, so the window is told its layout without it.
    again
        .window(&call, &json!({"action": "close", "tab": "t1"}))
        .unwrap();
    until("and goes", || !listed("t1"));
    again.shutdown();
    let _ = std::fs::remove_dir_all(&home);
}
