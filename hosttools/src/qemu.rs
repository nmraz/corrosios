use std::path::{Path, PathBuf};
use std::{fs, vec};

use anyhow::{Context, Result};
use xshell::{cmd, Shell, TempDir};

use crate::config;

pub struct QemuOptions<'a> {
    pub image_path: &'a Path,
    pub mem: &'a str,
    pub enable_gdbserver: bool,
    pub use_kvm: bool,
    pub headless: bool,
    pub serial: &'a str,
    pub additional_args: &'a [String],
}

pub fn run_qemu(sh: &Shell, opts: &QemuOptions<'_>) -> Result<()> {
    let firmware_paths = get_firmware_paths(sh)?;

    let disk = format!("file={},format=raw", opts.image_path.display());
    let uefi_flash = format!(
        "if=pflash,format=raw,readonly=on,file={}",
        firmware_paths.code.display()
    );
    let uefi_vars = format!(
        "if=pflash,format=raw,file={}",
        firmware_paths.vars.display()
    );

    let mut extra_args = vec![];

    if opts.enable_gdbserver {
        extra_args.extend(["-s", "-S"]);
    }

    if opts.use_kvm {
        extra_args.extend(["-accel", "kvm"]);
    }

    if opts.headless {
        extra_args.extend(["-nographic"]);
    }

    if !opts.serial.is_empty() {
        extra_args.extend(["-serial", opts.serial]);
    }

    extra_args.extend(opts.additional_args.iter().map(|arg| arg.as_str()));

    let mem = opts.mem;

    cmd!(
        sh,
        "qemu-system-x86_64 -m {mem} -drive {uefi_flash} -drive {uefi_vars} -drive {disk} {extra_args...}"
    )
    .run()
    .context("failed to start QEMU")
}

struct FirmwarePaths {
    _temp_dir: TempDir,
    code: PathBuf,
    vars: PathBuf,
}

fn get_firmware_paths(sh: &Shell) -> Result<FirmwarePaths> {
    let firmware_dir = config::get_workspace_root()?.join(config::QEMU_FIRMWARE_DIR);
    let temp_dir = sh
        .create_temp_dir()
        .context("failed to create temporary directory for UEFI variables")?;

    let vars = temp_dir.path().join("efivars.fd");
    fs::copy(firmware_dir.join(config::QEMU_FIRMWARE_VARS), &vars)
        .context("failed to copy UEFI variables to temporary directory")?;

    Ok(FirmwarePaths {
        _temp_dir: temp_dir,
        code: firmware_dir.join(config::QEMU_FIRMWARE_CODE),
        vars,
    })
}
