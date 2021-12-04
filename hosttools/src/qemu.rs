use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::config;

pub fn run_qemu(image_path: &Path, additional_args: &[String]) -> Result<()> {
    let mut cmd = Command::new("qemu-system-x86_64");

    let firmware_path = get_qemu_firwmare_path()?;

    let disk = format!("file={},format=raw", image_path.display());
    let uefi_flash = format!(
        "if=pflash,format=raw,readonly=on,file={}",
        firmware_path.display()
    );

    cmd.args(vec!["-drive", &uefi_flash, "-drive", &disk]);
    cmd.args(additional_args);

    cmd.spawn().context("failed to start QEMU")?.wait()?;
    Ok(())
}

fn get_qemu_firwmare_path() -> Result<PathBuf> {
    let mut path = config::get_workspace_root()?;
    path.push(config::QEMU_FIRMWARE_DIR);
    path.push(config::QEMU_FIRMWARE_NAME);
    Ok(path)
}
