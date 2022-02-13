use std::path::PathBuf;

use anyhow::{Context, Result};

pub const IMAGE_NAME: &str = "corrosios.img";

pub const BOOTLOADER_PACKAGE_NAME: &str = "efiboot";
pub const BOOTLOADER_PACKAGE_TARGET: &str = "x86_64-unknown-uefi";

pub const KERNEL_PACKAGE_NAME: &str = "kernel";
pub const KERNEL_PACKAGE_TARGET: &str = "kernel/kernel/x86_64-corrosios-kernel.json";

pub const QEMU_FIRMWARE_DIR: &str = "qemu/firmware/x64";
pub const QEMU_FIRMWARE_CODE: &str = "OVMF_CODE.fd";
pub const QEMU_FIRMWARE_VARS: &str = "OVMF_VARS.fd";

pub const GDB_INIT_SCRIPT: &str = "gdb/x64.gdb";

pub fn get_workspace_root() -> Result<PathBuf> {
    let hosttools_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = hosttools_dir
        .parent()
        .context("failed to get workspace root")?;
    Ok(workspace_root.to_owned())
}
