use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use anyhow::{Context, Result};
use tempfile::TempDir;

use crate::config;

pub struct QemuOptions<'a> {
    pub image_path: &'a Path,
    pub enable_gdbserver: bool,
    pub serial: &'a str,
    pub headless: bool,
}

pub struct QemuChild {
    _temp_dir: TempDir,
    child: Child,
}

impl QemuChild {
    pub fn wait(mut self) -> Result<()> {
        self.child.wait()?;
        Ok(())
    }
}

pub fn run_qemu(opts: &QemuOptions<'_>) -> Result<QemuChild> {
    let mut cmd = Command::new("qemu-system-x86_64");

    let firmware_paths = get_firmware_paths()?;

    let disk = format!("file={},format=raw", opts.image_path.display());
    let uefi_flash = format!(
        "if=pflash,format=raw,readonly=on,file={}",
        firmware_paths.code.display()
    );
    let uefi_vars = format!(
        "if=pflash,format=raw,file={}",
        firmware_paths.vars.display()
    );

    cmd.args(vec![
        "-accel",
        "kvm",
        "-drive",
        &uefi_flash,
        "-drive",
        &uefi_vars,
        "-drive",
        &disk,
    ]);

    if opts.enable_gdbserver {
        cmd.args(["-s", "-S"]);
    }

    if opts.headless {
        cmd.args(["-nographic"]);
    }

    if !opts.serial.is_empty() {
        cmd.args(["-serial", opts.serial]);
    }

    let child = cmd.spawn().context("failed to start QEMU")?;
    Ok(QemuChild {
        _temp_dir: firmware_paths.temp_dir,
        child,
    })
}

struct FirmwarePaths {
    temp_dir: TempDir,
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
        temp_dir,
        code: firmware_dir.join(config::QEMU_FIRMWARE_CODE),
        vars,
    })
}
