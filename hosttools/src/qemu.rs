use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use tempfile::TempDir;

use crate::config;

pub fn run_qemu(image_path: &Path, additional_args: &[String]) -> Result<()> {
    let mut cmd = Command::new("qemu-system-x86_64");

    let firmware_paths = get_firmware_paths()?;

    let disk = format!("file={},format=raw", image_path.display());
    let uefi_flash = format!(
        "if=pflash,format=raw,readonly=on,file={}",
        firmware_paths.code.display()
    );
    let uefi_vars = format!(
        "if=pflash,format=raw,file={}",
        firmware_paths.vars.display()
    );

    cmd.args(vec![
        "-drive",
        &uefi_flash,
        "-drive",
        &uefi_vars,
        "-drive",
        &disk,
    ]);
    cmd.args(additional_args);

    cmd.spawn().context("failed to start QEMU")?.wait()?;
    Ok(())
}

struct FirmwarePaths {
    _temp_dir: TempDir,
    code: PathBuf,
    vars: PathBuf,
}

fn get_firmware_paths() -> Result<FirmwarePaths> {
    let firmware_dir = config::get_workspace_root()?.join(config::QEMU_FIRMWARE_DIR);
    let temp_dir =
        tempfile::tempdir().context("failed to create temporary directory for UEFI variables")?;

    let vars = temp_dir.path().join("efivars.fd");
    fs::copy(firmware_dir.join(config::QEMU_FIRMWARE_VARS), &vars)
        .context("failed to copy UEFI variables to temporary directory")?;

    Ok(FirmwarePaths {
        _temp_dir: temp_dir,
        code: firmware_dir.join(config::QEMU_FIRMWARE_CODE),
        vars,
    })
}
