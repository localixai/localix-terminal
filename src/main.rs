//! The `llxterm` binary.

use std::process::ExitCode;

fn main() -> ExitCode {
    llxterm::cli::run(std::env::args().skip(1).collect())
}
