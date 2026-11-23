//! Where a run's results go and how its tables write numbers. A full run writes
//! `results/<name>.json`, which `check` holds to `budgets.json`, and `results/<name>.md`, the
//! tables `results/latest.md` gathers; a smoke run writes the same under `target/smoke/`, so it
//! never replaces the published figures.

use std::path::PathBuf;

use serde_json::Value;

use crate::Run;

/// The folder a run of this size writes its results to.
pub fn folder(run: Run) -> PathBuf {
    let bench = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    match run {
        Run::Full => bench.join("results"),
        Run::Smoke => bench.join("target/smoke"),
    }
}

/// One benchmark's results, written as JSON and as Markdown; the Markdown is printed too.
pub fn publish(run: Run, name: &str, json: &Value, markdown: &str) {
    let folder = folder(run);
    let written = std::fs::create_dir_all(&folder)
        .and_then(|()| {
            let text = serde_json::to_string_pretty(json).unwrap_or_default();
            std::fs::write(folder.join(format!("{name}.json")), text + "\n")
        })
        .and_then(|()| std::fs::write(folder.join(format!("{name}.md")), markdown));
    if let Err(why) = written {
        eprintln!("{}/{name}: not written ({why})", folder.display());
    }
    print!("{markdown}");
}

/// A throughput as a table says it: megabytes (10^6 bytes) a second.
pub fn speed(mbps: f64) -> String {
    match mbps {
        n if n.is_nan() => "–".into(),
        n if n < 10.0 => format!("{n:.1}"),
        n => format!("{n:.0}"),
    }
}

/// How many times faster `ours` is than `theirs`: `1.42x`, or `0.61x` where it is slower.
pub fn ratio(ours: f64, theirs: f64) -> String {
    match ours / theirs {
        n if n.is_finite() => format!("{n:.2}x"),
        _ => "–".into(),
    }
}

/// Bytes as a table says them: mebibytes, one decimal.
pub fn mebibytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / f64::from(1 << 20))
}

/// `text` as a cell of a Markdown table: a `|` in it would end the cell, a line break the row.
pub fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// A Markdown table's head: the names and the line under them; the first `text` columns are
/// left-aligned, the figures after them right-aligned.
pub fn head(names: &[&str], text: usize) -> String {
    let rule: Vec<&str> = (0..names.len())
        .map(|at| if at < text { "---" } else { "---:" })
        .collect();
    format!("| {} |\n|{}|\n", names.join(" | "), rule.join("|"))
}

/// A count of rounds in words: `one round`, `5 rounds`.
pub fn rounds(n: usize) -> String {
    match n {
        1 => "one round".into(),
        n => format!("{n} rounds"),
    }
}

/// A count as prose writes it: `10,000`.
pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (at, c) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
