//! What the benchmarks share: the terminal every core is given (120 by 40, 10,000 rows of
//! history), the output fed to it as a program's reads come (4 KiB at a time), a core as the
//! benchmarks drive it (`Engine`), its last screen read as text to compare one core with
//! another, and the two sizes of a run: the full one, whose results are committed under
//! `results/`, and the smoke run (`--quick` or `BENCH_SMALL=1`), small inputs and one round,
//! which proves every core works and writes only under `target/smoke/`.

pub mod child;
mod counting;
pub mod engines;
pub mod report;
pub mod workloads;

// The reader of the app's own recordings, shared with its tests: one parser of asciicast v2.
#[expect(
    dead_code,
    reason = "the workloads play a recording's output into a terminal of the bench's own size"
)]
#[path = "../../tests/support/cast.rs"]
mod cast;

pub use counting::{counters, fast_cores, wait_with_peak};

/// The terminal every core is given: rows, columns, and rows of history kept above the screen.
pub const ROWS: u16 = 40;
pub const COLUMNS: u16 = 120;
pub const HISTORY: usize = 10_000;
/// What one read of a program's terminal returns at most: a core is fed this much at a time.
pub const CHUNK: usize = 4096;

/// A terminal core as the benchmarks drive it.
pub trait Engine {
    /// Takes in a program's output, as a terminal reads it.
    fn feed(&mut self, bytes: &[u8]);
    /// The screen as it now shows, read as text.
    fn screen(&self) -> Screen;
}

/// `bytes` fed to `engine` a read at a time.
pub fn feed_all(engine: &mut dyn Engine, bytes: &[u8]) {
    for chunk in bytes.chunks(CHUNK) {
        engine.feed(chunk);
    }
}

/// A screen as text: where the cursor is (row and column, from 0) and each row in view, trailing
/// spaces and the blank rows at the end left out; a wide character once, a character with the
/// marks that combine with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub cursor: (u16, u16),
    pub rows: Vec<String>,
}

impl Screen {
    /// The screen of `rows` as an engine read them, put in the one form every engine's is
    /// compared in: a blank cell a core keeps as NUL is a space, trailing spaces go.
    pub fn new(cursor: (u16, u16), rows: impl IntoIterator<Item = String>) -> Self {
        let mut rows: Vec<String> = rows
            .into_iter()
            .map(|row| row.replace('\0', " ").trim_end().to_string())
            .collect();
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        Self { cursor, rows }
    }

    /// Where `other` first differs from this screen, in a few words; `None` when it is the same.
    pub fn differs(&self, other: &Screen) -> Option<String> {
        if self.cursor != other.cursor {
            return Some(format!("cursor {:?}, not {:?}", other.cursor, self.cursor));
        }
        let rows = self.rows.len().max(other.rows.len());
        let blank = String::new();
        (0..rows).find_map(|at| {
            let (ours, theirs) = (
                self.rows.get(at).unwrap_or(&blank),
                other.rows.get(at).unwrap_or(&blank),
            );
            (ours != theirs).then(|| format!("row {at}: {theirs:?}, not {ours:?}"))
        })
    }

    /// A hash of the whole screen, the same on every run and machine (FNV-1a): what the quick
    /// loop compares to tell a change that draws otherwise.
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let cursor = format!("{:?}", self.cursor);
        let text = self.rows.join("\n");
        for byte in cursor.bytes().chain(text.bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
        hash
    }
}

/// How big a run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// The published run: 16 MiB a workload, the median of 5 rounds after one to warm up.
    Full,
    /// The smoke run: 256 KiB a workload, one round, results under `target/smoke/`.
    Smoke,
}

impl Run {
    /// The run the command line asks for: `--quick` or `BENCH_SMALL=1` is the smoke run.
    pub fn from_args() -> Self {
        let quick = std::env::args().any(|arg| arg == "--quick")
            || std::env::var("BENCH_SMALL").is_ok_and(|value| value == "1");
        match quick {
            true => Run::Smoke,
            false => Run::Full,
        }
    }

    /// The bytes each workload is made of.
    pub fn size(self) -> usize {
        match self {
            Run::Full => 16 << 20,
            Run::Smoke => 256 << 10,
        }
    }

    /// Rounds measured, and rounds run first and thrown away.
    pub fn rounds(self) -> (usize, usize) {
        match self {
            Run::Full => (5, 1),
            Run::Smoke => (1, 0),
        }
    }
}

/// The names given after the options: the workloads (or engines) a run is held to; empty is all.
pub fn chosen() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with("--"))
        .collect()
}
