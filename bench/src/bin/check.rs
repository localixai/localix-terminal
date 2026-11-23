//! The last run held to `budgets.json`: each rule read against the results it names
//! (`results/*.json`; with `--quick`, the smoke run's under `target/smoke/`), one line each, and
//! a failure exit when any is broken or has no results.

use std::process::ExitCode;

use llxterm_bench::Run;
use llxterm_bench::engines::OURS;
use llxterm_bench::report::folder;
use serde_json::Value;

fn read(run: Run, name: &str) -> Option<Value> {
    let text = std::fs::read_to_string(folder(run).join(name)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Each workload's engines from the throughput results.
fn workloads(throughput: &Value) -> Vec<(String, Vec<Value>)> {
    (throughput["workloads"].as_array().into_iter().flatten())
        .map(|w| {
            let name = w["name"].as_str().unwrap_or("?").to_string();
            (name, w["engines"].as_array().cloned().unwrap_or_default())
        })
        .collect()
}

fn mbps(engines: &[Value], name: &str) -> f64 {
    (engines.iter().find(|e| e["name"] == name))
        .and_then(|e| e["mbps"].as_f64())
        .unwrap_or(f64::NAN)
}

/// Where llxterm is slower than `min` times what `theirs` picks from a workload's engines.
fn slower(
    throughput: &Value,
    min: f64,
    theirs: impl Fn(&[Value]) -> Option<(f64, String)>,
) -> Vec<String> {
    (workloads(throughput).iter())
        .filter_map(|(workload, engines)| {
            let (speed, who) = theirs(engines)?;
            let ratio = mbps(engines, OURS) / speed;
            (ratio.is_nan() || ratio < min).then(|| format!("{workload} {ratio:.2}x {who}"))
        })
        .collect()
}

/// Whether `rule` holds, and what was found; `None` when its results are missing.
fn judge(run: Run, rule: &Value) -> Option<(bool, String)> {
    let id = rule["id"].as_str().unwrap_or("");
    let min = rule["minRatio"].as_f64().unwrap_or(1.0);
    Some(match id {
        "every-engine-runs" => {
            let throughput = read(run, "throughput.json")?;
            let memory = read(run, "memory.json")?;
            let mut failed: Vec<String> = (workloads(&throughput).iter())
                .flat_map(|(w, engines)| {
                    (engines.iter().filter(|e| !e["panicked"].is_null()))
                        .map(move |e| format!("{w}: {} panicked", e["name"]))
                })
                .collect();
            for w in memory["workloads"].as_array().into_iter().flatten() {
                for (name, peak) in w["peaks"].as_object().into_iter().flatten() {
                    if peak.is_null() {
                        failed.push(format!("{}: {name} failed in its process", w["name"]));
                    }
                }
            }
            (failed.is_empty(), format!("failed: {failed:?}"))
        }
        "same-screen" => {
            let throughput = read(run, "throughput.json")?;
            let held: Vec<&str> = (rule["workloads"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .collect();
            let differ: Vec<String> = (workloads(&throughput).iter())
                .filter(|(w, _)| held.contains(&w.as_str()))
                .flat_map(|(w, engines)| {
                    (engines.iter().filter(|e| e["sameScreen"] == false))
                        .map(move |e| format!("{w}: {} ({})", e["name"], e["differs"]))
                })
                .collect();
            (differ.is_empty(), format!("other screens: {differ:?}"))
        }
        "above-floor" | "above-reference" => {
            let throughput = read(run, "throughput.json")?;
            let floor = rule["engine"].as_str().unwrap_or("vt100").to_string();
            let below = slower(&throughput, min, |engines| {
                Some((mbps(engines, &floor), floor.clone()))
            });
            (below.is_empty(), format!("below {min}x: {below:?}"))
        }
        "reference-level" => {
            let throughput = read(run, "throughput.json")?;
            let below = slower(&throughput, min, |engines| {
                (engines.iter())
                    .filter(|e| e["ours"] == false && e["panicked"].is_null())
                    .filter_map(|e| Some((e["mbps"].as_f64()?, e["name"].as_str()?.to_string())))
                    .max_by(|a, b| a.0.total_cmp(&b.0))
            });
            (below.is_empty(), format!("below {min}x: {below:?}"))
        }
        "memory" => {
            let memory = read(run, "memory.json")?;
            let max = rule["maxMiB"].as_f64().unwrap_or(0.0);
            let over: Vec<String> = (memory["workloads"].as_array().into_iter().flatten())
                .filter_map(|w| {
                    let mib = w["added"][OURS].as_f64()? / f64::from(1 << 20);
                    (mib > max).then(|| format!("{} {mib:.1} MiB", w["name"]))
                })
                .collect();
            (over.is_empty(), format!("over {max} MiB: {over:?}"))
        }
        _ => (false, format!("no check for the rule {id:?}")),
    })
}

fn main() -> ExitCode {
    let run = Run::from_args();
    let budgets = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/budgets.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or(Value::Null);
    let rules = budgets["rules"].as_array().cloned().unwrap_or_default();
    let mut broken = rules.is_empty();
    for rule in &rules {
        let id = rule["id"].as_str().unwrap_or("?");
        match judge(run, rule) {
            Some((true, found)) => println!("ok      {id}: {found}"),
            Some((false, found)) => {
                broken = true;
                println!("BROKEN  {id}: {found}");
            }
            None => {
                broken = true;
                println!("MISSING {id}: no results; run the benchmark that writes them");
            }
        }
    }
    match broken {
        true => ExitCode::FAILURE,
        false => ExitCode::SUCCESS,
    }
}
