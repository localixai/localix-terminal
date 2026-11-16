//! What the app knows of the person's own setup for their coding agents, which the Store
//! installs into (`crate::catalog`) and the window looks for agents by (`browser/agents.rs`):
//! their login shell's PATH and the variables that move an agent's folder, a program of theirs
//! run within a deadline (`shell.rs`), and where each agent keeps its configuration and how a
//! file of it is read and written whole (`config.rs`). It writes nothing by itself: the Store
//! writes through it only on the person's yes. A caller passes the home and the variables it
//! was given, so a test names a temporary home and never the person's own.

mod config;
mod shell;

pub(crate) use config::{
    claude_dir, claude_json, config_file, json, opencode_server, shown, toml_value, write_over,
};
pub(crate) use shell::{Ran, failure, login_path, login_vars, run, which};
