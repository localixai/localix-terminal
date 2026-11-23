//! One engine on one workload, in a process of its own: each starts from a fresh heap, a core
//! that cannot share a binary with another runs in its own (`Home`), and the kernel keeps the
//! process's peak memory apart from every other's. The parent writes the workload to a file and
//! starts the engine's binary; the child either times it (`time`: the rounds, the instructions,
//! the last screen, as one line of JSON) or feeds it once from the file a read at a time (`feed`,
//! for its peak memory).

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

use serde_json::{Value, json};

use crate::engines::{self, Kind};
use crate::workloads::Workload;
use crate::{CHUNK, Run, Screen, counters, fast_cores, feed_all, wait_with_peak};

/// What the child did, as the parent reads it.
#[derive(Default)]
pub struct Outcome {
    /// MB/s of each round measured, slowest first.
    pub speeds: Vec<f64>,
    /// Instructions retired by the round that retired fewest; `None` where none are counted.
    pub instructions: Option<u64>,
    pub screen: Option<Screen>,
    /// The most memory the process held resident, in bytes.
    pub peak: Option<u64>,
    /// Why it failed, when it did: the panic's words, or what kept it from starting.
    pub failed: Option<String>,
}

/// The file the children read `workload` from, written for this run: the machine's own, under
/// `target/`, never committed.
pub fn workload_file(run: Run, workload: &Workload) -> io::Result<PathBuf> {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(match run {
        Run::Full => "target/workloads",
        Run::Smoke => "target/workloads-smoke",
    });
    std::fs::create_dir_all(&folder)?;
    let path = folder.join(format!("{}.bin", workload.name));
    std::fs::write(&path, &workload.bytes)?;
    Ok(path)
}

/// Runs `kind` on the workload in `file` in its binary: timed (`time`) or fed once (`feed`).
pub fn run(kind: &Kind, time: bool, file: &Path, run: Run) -> Outcome {
    let failed = |why: String| Outcome {
        failed: Some(why),
        ..Outcome::default()
    };
    let binary = match std::env::current_exe() {
        Ok(exe) => exe.with_file_name(kind.home.binary()),
        Err(why) => return failed(why.to_string()),
    };
    let (rounds, warm) = run.rounds();
    let started = Command::new(&binary)
        .arg(kind.name)
        .arg(if time { "time" } else { "feed" })
        .arg(file)
        .args([rounds.to_string(), warm.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match started {
        Ok(child) => child,
        Err(why) => return failed(format!("{} not started ({why})", binary.display())),
    };
    let (mut said, mut complained) = (String::new(), String::new());
    if let Some(out) = child.stdout.as_mut() {
        let _ = out.read_to_string(&mut said);
    }
    if let Some(err) = child.stderr.as_mut() {
        let _ = err.read_to_string(&mut complained);
    }
    let (ok, peak) = match wait_with_peak(&child) {
        Ok(waited) => waited,
        Err(why) => return failed(why.to_string()),
    };
    if !ok {
        let why = complained
            .lines()
            .filter(|l| !l.starts_with("note:"))
            .collect::<Vec<_>>();
        return failed(why.join(" ").trim().to_string());
    }
    let said: Value = serde_json::from_str(&said).unwrap_or(Value::Null);
    Outcome {
        speeds: (said["speeds"].as_array().into_iter().flatten())
            .filter_map(Value::as_f64)
            .collect(),
        instructions: said["instructions"].as_u64().filter(|n| *n > 0),
        screen: screen_of(&said["screen"]),
        peak: Some(peak),
        failed: None,
    }
}

fn screen_of(said: &Value) -> Option<Screen> {
    let cursor = said["cursor"].as_array()?;
    let at = |i: usize| u16::try_from(cursor.get(i)?.as_u64()?).ok();
    let rows = (said["rows"].as_array()?.iter()).filter_map(|row| Some(row.as_str()?.to_string()));
    Some(Screen::new((at(0)?, at(1)?), rows))
}

/// The child's side, the whole of an `engine` binary's `main`:
/// `engine <name> <time|feed> <file> <rounds> <warm>`.
pub fn serve() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [name, how, file, rounds, warm] = args.as_slice() else {
        eprintln!("usage: engine <name> <time|feed> <file> <rounds> <warm>");
        return ExitCode::FAILURE;
    };
    let Some(make) = engines::named(name).and_then(|kind| kind.make) else {
        eprintln!("{name}: not in this binary");
        return ExitCode::FAILURE;
    };
    let path = Path::new(file);
    let done = match how.as_str() {
        "time" => time(
            make,
            path,
            rounds.parse().unwrap_or(1),
            warm.parse().unwrap_or(0),
        ),
        _ => feed(make, path),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{}: {why}", path.display());
            ExitCode::FAILURE
        }
    }
}

/// The workload timed: `warm` rounds thrown away, then `rounds`, each in a new terminal; the
/// result printed as one line of JSON.
fn time(make: engines::Make, path: &Path, rounds: usize, warm: usize) -> io::Result<()> {
    fast_cores();
    let bytes = std::fs::read(path)?;
    let mut speeds = Vec::with_capacity(rounds);
    let mut fewest = u64::MAX;
    let mut screen = None;
    for round in 0..rounds + warm {
        let mut engine = make();
        let (before, _) = counters();
        let started = Instant::now();
        feed_all(engine.as_mut(), &bytes);
        let seconds = started.elapsed().as_secs_f64();
        let (after, _) = counters();
        if round >= warm {
            speeds.push(bytes.len() as f64 / seconds / 1e6);
            fewest = fewest.min(after - before);
        }
        screen = Some(engine.screen());
    }
    speeds.sort_by(f64::total_cmp);
    let screen = screen.map(|s| json!({ "cursor": [s.cursor.0, s.cursor.1], "rows": s.rows }));
    println!(
        "{}",
        json!({ "speeds": speeds, "instructions": fewest, "screen": screen })
    );
    Ok(())
}

/// The workload fed once from its file, a read at a time, as a program's output comes.
fn feed(make: engines::Make, path: &Path) -> io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut engine = make();
    let mut buffer = [0u8; CHUNK];
    loop {
        match file.read(&mut buffer)? {
            0 => break,
            read => engine.feed(&buffer[..read]),
        }
    }
    std::hint::black_box(&engine);
    Ok(())
}
