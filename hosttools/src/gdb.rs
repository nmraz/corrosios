use std::path::Path;

use anyhow::{Context, Result};
use xshell::{cmd, Shell};

use crate::config;
use crate::utils::run_interactive;

pub struct GdbOptions<'a> {
    pub kernel_binary: &'a Path,
    pub server: &'a str,
}

pub fn run_gdb(sh: &Shell, opts: &GdbOptions<'_>) -> Result<()> {
    let &GdbOptions {
        kernel_binary,
        server,
    } = opts;
    let gdb_init_script = config::get_workspace_root()?.join(config::GDB_INIT_SCRIPT);

    run_interactive(cmd!(
        sh,
        "rust-gdb {kernel_binary} -ex 'target remote '{server} -x {gdb_init_script}"
    ))
    .context("failed to start rust-gdb")
}
