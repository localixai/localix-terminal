//! The quick loop: what one change to llxterm's core did, the same on every run. Every workload
//! of the bench, 2 MiB each, fed 4 KiB at a time to the core alone and with a frame after every
//! read, counted rather than timed:
//!
//! - instructions retired per byte, as the kernel counts them for the process: the same work
//!   retires the same instructions on every run, so a change of a tenth of a percent is the code;
//! - CPU cycles per byte, which also pay for missed caches and branches and move a few percent
//!   from run to run: the fewest of three rounds, on the fast cores;
//! - a fingerprint of the last screen: a change that draws otherwise says so, and the run exits
//!   with 2.
//!
//! ```sh
//! cargo run --release                    # in bench/quick: this tree against the run saved last
//! cargo run --release -- --save          # and keep this run as the one to compare with
//! cargo run --release -- --save-as start # keep it under a name of its own
//! cargo run --release -- --against start # compare with that one instead
//! ```
//!
//! The saved runs are `bench/target/quick-<name>.json` (`base` by default): the machine's own,
//! never committed. The published figures still come from `run.sh`. On other systems than macOS
//! there are no counters, and only the fingerprints are compared.

use std::path::PathBuf;
use std::process::ExitCode;

use llxterm_bench::engines::{self, Role};
use llxterm_bench::{Run, counters, fast_cores, feed_all, workloads};
use serde_json::{Map, Value, json};

/// The bytes of each workload, and the rounds of each of which the fewest cycles are kept:
/// enough that a terminal's setup is lost in it, few enough for a run of seconds. The smoke run
/// (`--quick` or `BENCH_SMALL=1`) is the bench's: 256 KiB, one round, only to see it work.
fn size(run: Run) -> (usize, usize) {
    match run {
        Run::Full => (2 << 20, 3),
        Run::Smoke => (run.size(), 1),
    }
}

/// The saved run called `name`.
fn saved(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../target/quick-{name}.json"))
}

/// The value after `flag` on the command line.
fn option(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let at = args.iter().position(|arg| arg == flag)?;
    args.get(at + 1).cloned()
}

/// Every row of this run: `workload / engine` to its counts and fingerprint.
fn measure(run: Run) -> Map<String, Value> {
    let (size, rounds) = size(run);
    let mut rows = Map::new();
    let kinds: Vec<_> = (engines::all().into_iter())
        .filter(|kind| kind.role == Role::Ours)
        .filter_map(|kind| Some((kind.name, kind.make?)))
        .collect();
    for workload in workloads::chosen(&[], size) {
        let bytes = workload.bytes.len() as f64;
        for (name, make) in &kinds {
            let (mut instructions, mut cycles) = (u64::MAX, u64::MAX);
            let mut fingerprint = 0;
            for _ in 0..rounds {
                let mut engine = make();
                let (i0, c0) = counters();
                feed_all(engine.as_mut(), &workload.bytes);
                let (i1, c1) = counters();
                instructions = instructions.min(i1 - i0);
                cycles = cycles.min(c1 - c0);
                fingerprint = engine.screen().fingerprint();
            }
            rows.insert(
                format!("{} / {name}", workload.name),
                json!({
                    "instructions": instructions as f64 / bytes,
                    "cycles": cycles as f64 / bytes,
                    "fingerprint": format!("{fingerprint:016x}"),
                }),
            );
        }
    }
    rows
}

/// `now` against `then` in percent, signed: `-3.1%` is less work.
fn change(now: f64, then: Option<f64>) -> String {
    match then {
        Some(then) if then > 0.0 => format!("{:+.2}%", (now - then) / then * 100.0),
        Some(_) | None => "–".into(),
    }
}

fn main() -> ExitCode {
    fast_cores();
    let against = option("--against").unwrap_or_else(|| "base".into());
    let run = Run::from_args();
    let before: Option<Value> = std::fs::read_to_string(saved(&against))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .filter(|_| run == Run::Full);
    let rows = measure(run);
    let mut drew_otherwise = Vec::new();
    println!(
        "{:<34} {:>10} {:>9} {:>10} {:>9}  screen",
        "workload / engine", "instr/B", "change", "cycles/B", "change"
    );
    for (name, row) in &rows {
        let then = before.as_ref().map(|b| &b[name.as_str()]);
        let figure = |of: &Value, what: &str| of[what].as_f64();
        let (instructions, cycles) = (
            figure(row, "instructions").unwrap_or(f64::NAN),
            figure(row, "cycles").unwrap_or(f64::NAN),
        );
        let screen = match then.map(|t| &t["fingerprint"]) {
            Some(Value::String(old)) if *old != row["fingerprint"] => {
                drew_otherwise.push(name.clone());
                "DIFFERS"
            }
            Some(Value::String(_)) => "same",
            Some(_) | None => "–",
        };
        println!(
            "{name:<34} {instructions:>10.1} {:>9} {cycles:>10.1} {:>9}  {screen}",
            change(instructions, then.and_then(|t| figure(t, "instructions"))),
            change(cycles, then.and_then(|t| figure(t, "cycles"))),
        );
    }
    // A smoke run's counts are of other inputs: never kept, never compared.
    let keep = (run == Run::Full)
        .then(|| option("--save-as"))
        .flatten()
        .or_else(|| {
            let save = std::env::args().any(|arg| arg == "--save");
            (save && run == Run::Full).then(|| "base".to_string())
        });
    if let Some(name) = keep {
        let text = serde_json::to_string_pretty(&Value::Object(rows)).unwrap_or_default();
        match std::fs::write(saved(&name), text) {
            Ok(()) => println!("saved as {name}"),
            Err(why) => eprintln!("{}: {why}", saved(&name).display()),
        }
    }
    if before.is_none() {
        println!("no saved run {against:?} to compare with: --save keeps this one");
    }
    match drew_otherwise.is_empty() {
        true => ExitCode::SUCCESS,
        false => {
            eprintln!("the last screen differs from the saved run's: {drew_otherwise:?}");
            ExitCode::from(2)
        }
    }
}
