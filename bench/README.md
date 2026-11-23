# Benchmark

How fast llxterm's terminal core takes in a program's output and what it holds, measured the same
way for it and for the cores other terminals are built on. This is what the core is held to:
[`budgets.json`](budgets.json) states the rules, `cargo run --release --bin check` fails when a
run breaks one, and every run's results are committed under [`results/`](results/).

The latest numbers: [`results/latest.md`](results/latest.md).

The canvas that draws a frame is measured apart, by `tests/view_bench.rs`; this bench stops where
a view's frame is made.

## What is compared

| Engine | What it is |
| --- | --- |
| `llxterm` | This tree's core: libghostty-vt behind `llxterm::terminal::Core`, fed with `Core::feed` |
| `llxterm, framed` | The same, and a view's frame (`Render::frame`) taken after every read, as the app takes one each time it wakes |
| `alacritty_terminal` | alacritty's core with its own `vte` parser, the core several Rust terminals embed |
| `wezterm-term` | wezterm's core, from its repository at a pinned commit (it is not released on crates.io) |
| `rio-vt` | rio's core without its renderer, window and clipboard (`rio-backend` is this crate and those) |
| `vt100` | A small parser and screen: the floor |

## What is measured

| Benchmark | What it runs | Results |
| --- | --- | --- |
| `throughput` | Every workload into every engine: MB/s (the median of 5 rounds after one thrown away, and the best), the instructions retired per byte, and the last screen held to llxterm's | `throughput.json`, `throughput.md` |
| `memory` | Every workload into every engine once, in a process of its own: the most memory the process held resident, less what the same binary holds feeding nothing | `memory.json`, `memory.md` |

The workloads, made by `src/workloads*` from fixed seeds, 16 MiB each, every one ending where a
line or a frame ends:

| Workload | What a program writes |
| --- | --- |
| `ascii` | Numbered lines, as `seq` prints them |
| `dense` | Every cell its own colour: a truecolor foreground and a 256-colour ground |
| `scroll-region` | Lines written at the foot of a scroll region, scrolled both ways now and then |
| `cursor` | Short words all over the screen, the cursor moved before each, the rest of a row erased now and then |
| `unicode` | CJK, emoji joined by ZWJ, with a skin tone or VS16, flags, letters with combining marks |
| `tui` | Full-screen frames on the other screen, each in a synchronized update (mode 2026) |
| `long-line` | Lines of 64 KiB, each wrapped over 546 rows |
| `recorded` | The app's recorded sessions (`tests/fixtures/terminal/*.cast`) one after another, their resizes left out |

## How

- **The same terminal and the same reads.** Every engine is a fresh terminal of 120 by 40 with
  10,000 rows of history, fed the workload 4 KiB at a time, as a program's reads come.
- **Built for speed, alike.** Every core is built with `opt-level = 3`, fat LTO and one codegen
  unit. The app itself ships built for size; libghostty-vt is a prebuilt library either way.
- **A process each.** Each engine runs each workload in a process of its own (`src/child.rs`), so
  each starts from a fresh heap and its peak memory is its own. `rio-vt` runs in a binary of its
  own (`apart/`, `engine-apart`): its copy of the simdutf library has the same symbols as the one
  inside libghostty-vt, and in one binary the linker would keep one copy for both.
- **Time.** The median of 5 rounds after one thrown away, each round a new terminal; only the
  feeding is timed, from a workload already in memory, on the fast cores.
- **Instructions** retired per byte, as the kernel counts them for the process
  (`proc_pid_rusage`, macOS): the same work retires the same instructions on every run, so this
  column moves only when the code does.
- **Memory.** `wait4` gives a child's peak resident memory; the child reads the workload from a
  file a read at a time. Each binary's process feeding nothing is its floor, subtracted.
- **The last screen.** After its last round every engine's screen is read as text (a wide
  character once, combining marks with their letter, trailing spaces left out) with its cursor,
  and held to llxterm's. Every difference is listed with the first row it is found in; on
  `ascii` the budgets require none, so the engines are known to do the same work.

Timings depend on the machine; `results/latest.md` says which one ran them. The ratios between
the engines move far less than the times do.

## Run

```sh
./run.sh                                  # everything, results/ written, then held to budgets.json
./run.sh --quick                          # the smoke run: 256 KiB, one round, target/smoke/ only
cargo run --release --bin throughput -- tui unicode     # some workloads only
cargo run --release --bin memory
cargo run --release --bin check           # the results in results/, held to budgets.json again
(cd quick && cargo run --release)         # the quick loop: this tree against the run saved last
```

`run.sh` builds both binaries first; to run `throughput` or `memory` alone, build `apart/` too
(`cd apart && cargo build --release`), or `rio-vt` is reported as not started. The full run takes
the machine's whole attention for a few minutes: nothing else should run meanwhile.

## The quick loop

A change to the core is measured first by `quick/`, a crate of its own with llxterm alone (the
references are the `references` feature, off there), which shares the bench's `target/`:

```sh
cd quick && cargo run --release              # this tree against the run saved last
cargo run --release -- --save                # keep this run as the one to compare with
cargo run --release -- --save-as start       # or under a name: compare with --against start
```

Every workload, 2 MiB each, into `llxterm` and `llxterm, framed`, counted rather than timed: the
instructions retired per byte, the same on every run; the CPU cycles per byte, the fewest of
three rounds; and a fingerprint of the last screen, so a change that draws otherwise says so and
the run exits with 2. A change is kept when it does less work and the fingerprints hold; the
published figures still come from `run.sh`.

## Budgets

| Rule | Holds |
| --- | --- |
| `every-engine-runs` | Every engine takes in every workload without a panic, timed and in a process of its own |
| `same-screen` | On `ascii` every engine ends on llxterm's screen and cursor |
| `above-floor` | On every workload llxterm is at least as fast as `vt100` |
| `above-reference` | On every workload llxterm is at least as fast as `alacritty_terminal` |
| `reference-level` | On every workload llxterm is at least 0.6 of the fastest reference (the gap left is inside the core library; `budgets.json` says why) |
| `memory` | llxterm's core holds at most 24 MiB over its process's floor, on every workload |

A number changes only together with its reason, in the change that moves it.
