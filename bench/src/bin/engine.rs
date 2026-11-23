//! One engine on one workload, started by `throughput` and `memory` for each pair: the engines
//! this crate links (every one but rio-vt, whose binary is `apart/`'s).

fn main() -> std::process::ExitCode {
    llxterm_bench::child::serve()
}
