//! The goal helper's benchmark: the tasks of `bench/tasks.json` on the demo sites, each run
//! through the desk in a real window as an agent's calls would be, scored by a check of the
//! page's own state, and written down in `bench/` to tell whether a change made it better.
//! It costs OpenRouter requests, so it runs only when asked:
//! `LLX_DESK_BENCH=1 cargo test --release --test bench -- [--repeat N] [--note TEXT] [task…]`.
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::disallowed_methods,
    clippy::cast_precision_loss,
    clippy::struct_excessive_bools,
    reason = "a benchmark's own `main`: it prints its table, asks git and date for the build and \
              the time, averages counts, and a run's record is the facts it checked"
)]

use std::path::Path;
use std::sync::Arc;

use llx_desk::browser;
use llx_desk::desk::{Call, Desk, Quiet};

#[path = "bench/record.rs"]
mod record;
#[path = "bench/run.rs"]
mod run;

/// What the command line asked for.
struct Asked {
    repeat: usize,
    note: String,
    filters: Vec<String>,
}

fn asked() -> Asked {
    let mut asked = Asked {
        repeat: 3,
        note: String::new(),
        filters: vec![],
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repeat" => asked.repeat = args.next().and_then(|n| n.parse().ok()).unwrap_or(3),
            "--note" => asked.note = args.next().unwrap_or_default(),
            other if other.starts_with('-') => {}
            other => asked.filters.push(other.to_string()),
        }
    }
    asked
}

fn main() {
    // Every request is paid: only when asked for.
    if std::env::var_os("LLX_DESK_BENCH").is_none() {
        return;
    }
    let asked = asked();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tasks = match run::tasks(&root.join("bench/tasks.json")) {
        Ok(tasks) => tasks,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let chosen: Vec<run::Task> = (tasks.into_iter())
        .filter(|t| {
            asked.filters.is_empty() || asked.filters.iter().any(|f| t.id.contains(f.as_str()))
        })
        .collect();
    let Some(key) = std::env::var("OPENROUTER_API_KEY")
        .ok()
        .or_else(llx_desk::key::find)
    else {
        eprintln!("the benchmark needs an OpenRouter key: `llx-desk key`, or OPENROUTER_API_KEY");
        std::process::exit(2);
    };
    let model = std::env::var("DESK_DECISION_MODEL").unwrap_or_else(|_| "default".into());
    // The window is a shown one, never over what the person is watching.
    // SAFETY: the first thing the process does with its environment, before any thread starts.
    unsafe { std::env::set_var("DESK_WINDOWS", "unseen") };
    let shots = std::env::temp_dir().join("llx-desk-bench-shots");
    browser::host(
        || std::process::exit(130),
        move |browser| {
            let desk = Desk::new(Some(browser), true, shots.clone(), Arc::new(Quiet));
            let call = Call {
                owner: "bench",
                cwd: root,
                vars: None,
                key: Some(key.trim()),
                progress: None,
            };
            let mut runs = vec![];
            for task in &chosen {
                for repeat in 1..=asked.repeat {
                    let done = run::run(&desk, &call, task, repeat);
                    let mark = match (&done.error, done.passed) {
                        (Some(error), _) => format!("error: {error}"),
                        (None, true) => "pass".into(),
                        (None, false) => format!("FAIL ({})", done.outcomes.join(" / ")),
                    };
                    println!("{} #{repeat}: {mark}", task.id);
                    runs.push(done);
                }
            }
            let score = record::score(&runs);
            let (when, build) = (record::now(), record::build(root));
            let meta = (
                when.as_str(),
                build.as_str(),
                model.as_str(),
                asked.note.as_str(),
            );
            match record::write(&root.join("bench"), meta, &runs, &score) {
                Ok(previous) => {
                    println!("\n{}", record::table(&runs, &score));
                    if let Some(previous) = previous {
                        println!("\nthe run before:\n{previous}");
                    }
                }
                Err(error) => eprintln!("not written: {error}"),
            }
            0
        },
    );
}
