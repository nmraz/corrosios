use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use cargo_metadata::Message;
use fatfs::{FileSystem, FormatVolumeOptions, FsOptions, ReadWriteSeek};
use fscommon::StreamSlice;
use gpt::disk::LogicalBlockSize;
use gpt::mbr::ProtectiveMBR;
use gpt::{GptConfig, GptDisk};

const KB: u64 = 1024;
const MB: u64 = KB * KB;

const LB_SIZE: u64 = 512;

const EFI_PARTITION_SIZE: u64 = 10 * MB;
const DISK_SIZE: u64 = EFI_PARTITION_SIZE + 64 * KB;

const EFI_PACKAGE_NAME: &str = "efiapp";
const EFI_PACKAGE_TARGET: &str = "x86_64-unknown-uefi";

pub fn create_disk_image(additional_args: &[String]) -> Result<PathBuf> {
    cross_run_all("build", additional_args)?;
    let binary = efi_binary_path(additional_args)?;

    let image_path = binary.with_extension("img");

    let mut disk = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&image_path)?;
    disk.set_len(DISK_SIZE)?;

    let mut gdisk = format_gpt(&mut disk).context("failed to format GPT disk")?;
    let (start, end) = add_efi_partition(&mut gdisk)?;
    gdisk.write().context("failed to flush partition table")?;

    let efi_part_data = StreamSlice::new(disk, start, end)?;
    format_efi_partition(efi_part_data, &binary).context("failed to write EFI system partition")?;

    Ok(image_path)
}

pub fn cross_run_all(subcommand: &str, additional_args: &[String]) -> Result<()> {
    cross_run(
        subcommand,
        EFI_PACKAGE_NAME,
        EFI_PACKAGE_TARGET,
        additional_args,
    )
}

fn efi_binary_path(additional_args: &[String]) -> Result<PathBuf> {
    built_binary_path(EFI_PACKAGE_NAME, EFI_PACKAGE_TARGET, additional_args)
}

fn cross_run(
    subcommand: &str,
    package_name: &str,
    target: &str,
    additional_args: &[String],
) -> Result<()> {
    let mut cmd = freestanding_cross_cmd(subcommand, package_name, target, additional_args);
    if !cmd.status()?.success() {
        bail!("`cargo {}` failed", subcommand);
    }
    Ok(())
}

fn built_binary_path(
    package_name: &str,
    target: &str,
    additional_args: &[String],
) -> Result<PathBuf> {
    let mut cmd = freestanding_cross_cmd("build", package_name, target, additional_args);
    cmd.arg("--message-format=json");

    let output = cmd.output()?.stdout;

    for message in Message::parse_stream(&output[..]) {
        if let Message::CompilerArtifact(artifact) = message? {
            if let Some(path) = artifact.executable {
                return Ok(path.into());
            }
        }
    }

    bail!("failed to extract binary path")
}

fn freestanding_cross_cmd(
    subcommand: &str,
    package_name: &str,
    target: &str,
    additional_args: &[String],
) -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args([
        subcommand,
        "-p",
        package_name,
        "--target",
        target,
        "-Zbuild-std=core,alloc",
        "-Zbuild-std-features=compiler-builtins-mem",
    ]);
    cmd.args(additional_args);

    cmd
}

fn format_gpt(disk: &mut File) -> Result<GptDisk<'_>> {
    let mbr =
        ProtectiveMBR::with_lb_size(u32::try_from(DISK_SIZE / LB_SIZE - 1).unwrap_or(0xffffffff));
    mbr.overwrite_lba0(disk).context("failed to write MBR")?;

    let mut gdisk = GptConfig::new()
        .initialized(false)
        .writable(true)
        .logical_block_size(LogicalBlockSize::Lb512)
        .create_from_device(Box::new(disk), None)?;

    gdisk
        .update_partitions(BTreeMap::new())
        .context("failed to initialize GPT")?;

    Ok(gdisk)
}

fn add_efi_partition(gdisk: &mut GptDisk<'_>) -> Result<(u64, u64)> {
    let id = gdisk
        .add_partition(
            "EFI System Partition",
            EFI_PARTITION_SIZE,
            gpt::partition_types::EFI,
            0,
        )
        .context("failed to create EFI system partition")?;

    let part = gdisk
        .partitions()
        .get(&id)
        .ok_or_else(|| anyhow!("failed to get EFI system partition"))?;

    let start = part.bytes_start(LogicalBlockSize::Lb512)?;
    let end = start + part.bytes_len(LogicalBlockSize::Lb512)?;

    Ok((start, end))
}

fn format_efi_partition(mut partition: impl ReadWriteSeek, efi_binary: &Path) -> Result<()> {
    fatfs::format_volume(&mut partition, FormatVolumeOptions::new())?;
    let fs = FileSystem::new(partition, FsOptions::new())?;

    let efi_dir = fs.root_dir().create_dir("efi")?;
    let boot_dir = efi_dir.create_dir("boot")?;
    let mut boot_file = boot_dir.create_file("bootx64.efi")?;

    io::copy(&mut File::open(efi_binary)?, &mut boot_file)?;

    let mut test_file = fs.root_dir().create_file("test.txt")?;
    write!(test_file, "Hello, world!")?;

    Ok(())
}
