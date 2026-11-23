//! How fast each core takes in a program's output: every workload fed 4 KiB at a time into a
//! fresh 120 by 40 terminal of each engine, each engine in a process of its own (`child.rs`),
//! timed by the clock (the median and the best of the rounds, after one thrown away) and counted
//! in instructions retired (the same on every run); then each engine's last screen held to
//! llxterm's, every difference said.
//!
//! ```sh
//! cargo run --release --bin throughput             # the full run: results/throughput.*
//! cargo run --release --bin throughput -- --quick  # the smoke run: target/smoke/throughput.*
//! cargo run --release --bin throughput -- ascii tui  # some workloads only
//! ```

use std::fmt::Write;
use std::path::Path;
use std::process::ExitCode;

use llxterm_bench::engines::{self, Kind, OURS, Role};
use llxterm_bench::report::{self, cell, head, ratio, speed};
use llxterm_bench::workloads::{self, Workload};
use llxterm_bench::{CHUNK, COLUMNS, HISTORY, ROWS, Run, Screen, child};
use serde_json::{Value, json};

/// One engine on one workload.
struct Measured {
    name: &'static str,
    role: Role,
    median: f64,
    best: f64,
    /// Instructions retired per byte fed; `None` where the kernel does not count them.
    per_byte: Option<f64>,
    /// Why it failed, when it did: its panic, or what kept it from starting.
    panicked: Option<String>,
    screen: Option<Screen>,
}

/// `kind` on the workload in `file`, in a process of its own: the rounds the run asks for, each
/// in a terminal of its own.
fn measure(kind: &Kind, file: &Path, bytes: usize, run: Run) -> Measured {
    let outcome = child::run(kind, true, file, run);
    let speeds = &outcome.speeds;
    Measured {
        name: kind.name,
        role: kind.role,
        median: speeds.get(speeds.len() / 2).copied().unwrap_or(f64::NAN),
        best: speeds.last().copied().unwrap_or(f64::NAN),
        per_byte: outcome.instructions.map(|n| n as f64 / bytes as f64),
        panicked: outcome.failed,
        screen: outcome.screen,
    }
}

/// Every engine but the baseline, on `workload`; each one's screen against llxterm's.
fn workload_json(workload: &Workload, measured: &[Measured]) -> Value {
    let ours = measured
        .iter()
        .find(|m| m.name == OURS)
        .and_then(|m| m.screen.as_ref());
    let engines: Vec<Value> = measured
        .iter()
        .map(|m| {
            let differs = match (ours, &m.screen) {
                (Some(ours), Some(theirs)) => ours.differs(theirs),
                _ => Some("no screen to compare".into()),
            };
            json!({
                "name": m.name,
                "ours": m.role == Role::Ours,
                "mbps": m.median,
                "bestMbps": m.best,
                "instructionsPerByte": m.per_byte,
                "panicked": m.panicked,
                "sameScreen": differs.is_none(),
                "differs": differs,
            })
        })
        .collect();
    json!({
        "name": workload.name,
        "about": workload.about,
        "bytes": workload.bytes.len(),
        "engines": engines,
    })
}

/// The fastest reference's median on a workload's JSON, and its name.
fn fastest_reference(workload: &Value) -> Option<(f64, String)> {
    (workload["engines"].as_array()?.iter())
        .filter(|e| e["ours"] == false && e["panicked"].is_null())
        .filter_map(|e| Some((e["mbps"].as_f64()?, e["name"].as_str()?.to_string())))
        .max_by(|a, b| a.0.total_cmp(&b.0))
}

/// A table of one figure of every engine, a row a workload, with llxterm's ratios after it.
fn table(workloads: &[Value], names: &[&str], field: &str, ratios: bool) -> String {
    let mut columns = vec!["workload"];
    columns.extend(names);
    if ratios {
        columns.extend([
            "llxterm vs alacritty_terminal",
            "llxterm vs the fastest other",
        ]);
    }
    let mut out = head(&columns, 1);
    for workload in workloads {
        let figure = |name: &str| {
            (workload["engines"].as_array().into_iter().flatten())
                .find(|e| e["name"] == name)
                .and_then(|e| e[field].as_f64())
                .unwrap_or(f64::NAN)
        };
        let _ = write!(out, "| {}", workload["name"].as_str().unwrap_or("?"));
        for name in names {
            let shown = match field {
                "instructionsPerByte" => format!("{:.1}", figure(name)),
                _ => speed(figure(name)),
            };
            let _ = write!(out, " | {shown}");
        }
        if ratios {
            let fastest = fastest_reference(workload);
            let (theirs, who) = fastest.unwrap_or((f64::NAN, "–".into()));
            let _ = write!(
                out,
                " | {} | {} ({who})",
                ratio(figure(OURS), figure("alacritty_terminal")),
                ratio(figure(OURS), theirs)
            );
        }
        out.push_str(" |\n");
    }
    out
}

/// What the screens said: which engines ended on another screen than llxterm's, and where.
fn screens(workloads: &[Value]) -> String {
    let mut out = String::new();
    for workload in workloads {
        let name = workload["name"].as_str().unwrap_or("?");
        let engines = workload["engines"].as_array().cloned().unwrap_or_default();
        let others: Vec<String> = (engines.iter())
            .filter(|e| e["sameScreen"] == false)
            .map(|e| {
                let why = (e["panicked"].as_str())
                    .map(|p| format!("panicked: {p}"))
                    .or_else(|| e["differs"].as_str().map(str::to_string))
                    .unwrap_or_default();
                format!("{} ({})", e["name"].as_str().unwrap_or("?"), cell(&why))
            })
            .collect();
        let said = match others.is_empty() {
            true => "every engine ends on llxterm's screen and cursor".to_string(),
            false => format!("other than llxterm's: {}", others.join("; ")),
        };
        let _ = writeln!(out, "| {name} | {said} |");
    }
    head(&["workload", "last screen"], 2) + &out
}

fn markdown(run: Run, workloads: &[Value], names: &[&str]) -> String {
    let (rounds, warm) = run.rounds();
    let mut out = String::new();
    let thrown = match warm {
        0 => String::new(),
        n => format!(" after {} thrown away", report::rounds(n)),
    };
    let _ = writeln!(
        out,
        "Each workload fed {} KiB at a time into a fresh {COLUMNS} by {ROWS} terminal with {} \
         rows of history, each engine in a process of its own; MB/s is 10^6 bytes a second, \
         the median of {}{thrown}.\n",
        CHUNK / 1024,
        report::thousands(HISTORY),
        report::rounds(rounds),
    );
    out.push_str(&head(&["workload", "what it is", "size"], 2));
    for w in workloads {
        let size = w["bytes"].as_u64().unwrap_or(0);
        let _ = writeln!(
            out,
            "| {} | {} | {} MiB |",
            w["name"].as_str().unwrap_or("?"),
            cell(w["about"].as_str().unwrap_or("")),
            report::mebibytes(size)
        );
    }
    out.push_str("\n### MB/s, the median\n\n");
    out.push_str(&table(workloads, names, "mbps", true));
    out.push_str("\n### MB/s, the best round\n\n");
    out.push_str(&table(workloads, names, "bestMbps", false));
    if cfg!(target_os = "macos") {
        out.push_str("\n### Instructions retired per byte (the same on every run)\n\n");
        out.push_str(&table(workloads, names, "instructionsPerByte", false));
    }
    out.push_str("\n### The last screen, against llxterm's\n\n");
    out.push_str(&screens(workloads));
    out
}

fn main() -> ExitCode {
    let run = Run::from_args();
    let kinds: Vec<Kind> = (engines::all().into_iter())
        .filter(|kind| kind.role != Role::Baseline)
        .collect();
    let names: Vec<&str> = kinds.iter().map(|kind| kind.name).collect();
    let chosen = llxterm_bench::chosen();
    let mut results = Vec::new();
    for workload in workloads::chosen(&chosen, run.size()) {
        let file = match child::workload_file(run, &workload) {
            Ok(file) => file,
            Err(why) => {
                eprintln!("{}: not written ({why})", workload.name);
                return ExitCode::FAILURE;
            }
        };
        let measured: Vec<Measured> = (kinds.iter())
            .map(|kind| {
                eprintln!("{}: {}", workload.name, kind.name);
                measure(kind, &file, workload.bytes.len(), run)
            })
            .collect();
        results.push(workload_json(&workload, &measured));
    }
    let (rounds, warm) = run.rounds();
    let json = json!({
        "smoke": run == Run::Smoke,
        "rounds": rounds,
        "warm": warm,
        "chunk": CHUNK,
        "rows": ROWS,
        "columns": COLUMNS,
        "history": HISTORY,
        "workloads": results,
    });
    report::publish(run, "throughput", &json, &markdown(run, &results, &names));
    ExitCode::SUCCESS
}
