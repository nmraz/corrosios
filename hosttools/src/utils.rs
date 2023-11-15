use std::process::Command;

use anyhow::{ensure, Result};
use xshell::Cmd;

pub fn run_interactive(cmd: Cmd<'_>) -> Result<()> {
    // We basically emulate `Cmd::run` here because `run` pipes stdin to its child (which isn't what
    // we want for an interactive process that creates/uses a TTY).
    eprintln!("$ {cmd}");
    let mut cmd: Command = cmd.into();
    let status = cmd.status()?;
    ensure!(status.success(), "command exited with status {status}");
    Ok(())
}
