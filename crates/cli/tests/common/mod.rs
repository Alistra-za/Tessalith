//! Shared by the CLI test files (mod common;).
use std::process::Command;

/// The sdc binary with APPDATA pointed at an empty folder, so the developer's real user config is never read.
pub fn sdc() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sdc"));
    c.env(
        "APPDATA",
        std::env::temp_dir().join("sdc-test-appdata-none"),
    );
    c
}
