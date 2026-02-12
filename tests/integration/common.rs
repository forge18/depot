//! Common utilities for integration tests

use std::process::Command;

pub fn depot_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_depot"))
}
