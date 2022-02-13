use std::path::Path;
use std::process::{Child, Command};

use anyhow::{Context, Result};

use crate::config;

pub struct GdbOptions<'a> {
    pub kernel_binary: &'a Path,
    pub server_port: u16,
}

pub fn run_gdb(opts: &GdbOptions<'_>) -> Result<Child> {
    let gdb_init_script = config::get_workspace_root()?
        .join(config::GDB_INIT_SCRIPT)
        .display()
        .to_string();

    let mut cmd = Command::new("rust-gdb");

    cmd.arg(opts.kernel_binary);

    let localhost_target = format!("target remote localhost:{}", opts.server_port);
    cmd.args(["-ex", &localhost_target]);

    cmd.args(["-x", &gdb_init_script]);

    cmd.spawn().context("failed to start rust-gdb")
}
