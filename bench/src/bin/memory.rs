//! What each core holds: every engine fed every workload in a process of its own (`child.rs`),
//! which reads the workload from a file 4 KiB at a time as a program's reads come, and the most
//! memory that process ever held resident, as the kernel kept it. Each binary's process feeding
//! nothing is its floor (`none` for `engine`, `none, apart` for `engine-apart`); what an engine
//! holds is its peak over the floor of the binary it ran in.
//!
//! ```sh
//! cargo run --release --bin memory             # the full run: results/memory.*
//! cargo run --release --bin memory -- --quick  # the smoke run: target/smoke/memory.*
//! ```

use std::fmt::Write;
use std::process::ExitCode;

use llxterm_bench::engines::{self, NONE, NONE_APART, OURS, Role, floor_of};
use llxterm_bench::report::{self, head, mebibytes};
use llxterm_bench::{Run, child, workloads};
use serde_json::{Value, json};

fn markdown(rows: &[Value], names: &[&str]) -> String {
    let mut out = String::from(
        "What a terminal holds, in MiB: the most memory a process held resident as it fed one \
         engine one workload from a file, 4 KiB at a time, less what the same binary holds \
         feeding nothing (its floor, the last columns).\n\n",
    );
    let mut columns = vec!["workload"];
    columns.extend(names);
    columns.extend([
        "llxterm vs alacritty_terminal",
        "floor: engine",
        "floor: engine-apart",
    ]);
    out.push_str(&head(&columns, 1));
    for row in rows {
        let added = |name: &str| row["added"][name].as_u64();
        let _ = write!(out, "| {}", row["name"].as_str().unwrap_or("?"));
        for name in names {
            let shown = added(name).map_or_else(|| "failed".into(), mebibytes);
            let _ = write!(out, " | {shown}");
        }
        let share = match (added(OURS), added("alacritty_terminal")) {
            (Some(ours), Some(theirs)) if theirs > 0 => {
                format!("{:.2}x", ours as f64 / theirs as f64)
            }
            _ => "–".into(),
        };
        let floor = |name: &str| {
            row["peaks"][name]
                .as_u64()
                .map_or_else(|| "–".into(), mebibytes)
        };
        let _ = writeln!(
            out,
            " | {share} | {} | {} |",
            floor(NONE),
            floor(NONE_APART)
        );
    }
    out
}

fn main() -> ExitCode {
    let run = Run::from_args();
    let kinds = engines::all();
    let names: Vec<&str> = (kinds.iter())
        .filter(|kind| kind.role != Role::Baseline)
        .map(|kind| kind.name)
        .collect();
    let mut rows = Vec::new();
    for workload in workloads::chosen(&llxterm_bench::chosen(), run.size()) {
        let file = match child::workload_file(run, &workload) {
            Ok(file) => file,
            Err(why) => {
                eprintln!("{}: not written ({why})", workload.name);
                return ExitCode::FAILURE;
            }
        };
        let mut peaks = serde_json::Map::new();
        let mut failures = serde_json::Map::new();
        for kind in &kinds {
            eprintln!("{}: {}", workload.name, kind.name);
            let outcome = child::run(kind, false, &file, run);
            peaks.insert(kind.name.into(), json!(outcome.peak));
            if let Some(why) = outcome.failed {
                failures.insert(kind.name.into(), json!(why));
            }
        }
        let mut added = serde_json::Map::new();
        for kind in kinds.iter().filter(|kind| kind.role != Role::Baseline) {
            let peak = |name: &str| peaks.get(name).and_then(Value::as_u64);
            let over = (peak(kind.name))
                .zip(peak(floor_of(kind.home)))
                .map(|(peak, floor)| peak.saturating_sub(floor));
            added.insert(kind.name.into(), json!(over));
        }
        rows.push(json!({
            "name": workload.name,
            "bytes": workload.bytes.len(),
            "peaks": peaks,
            "added": added,
            "failed": failures,
        }));
    }
    let json = json!({ "smoke": run == Run::Smoke, "workloads": rows });
    report::publish(run, "memory", &json, &markdown(&rows, &names));
    ExitCode::SUCCESS
}
