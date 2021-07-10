use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use argh::FromArgs;
use cargo_metadata::{camino::Utf8PathBuf, Message};
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

#[derive(FromArgs)]
/// Create a bootable UEFI image.
struct Args {
    #[argh(switch)]
    /// compile in release mode
    release: bool,
}

fn main() -> Result<()> {
    let args: Args = argh::from_env();

    let mut cargo_cmd = Command::new(env!("CARGO"));
    cargo_cmd.args([
        "build",
        "-p",
        "efiapp",
        "--target",
        "x86_64-unknown-uefi",
        "-Zbuild-std=core",
        "-Zbuild-std-features=compiler-builtins-mem",
    ]);

    if args.release {
        cargo_cmd.arg("--release");
    }

    if !cargo_cmd.status()?.success() {
        bail!("failed to build");
    }

    cargo_cmd.arg("--message-format=json");

    let binary = get_binary_path(&cargo_cmd.output()?.stdout)?;

    let binary: PathBuf = binary.into();
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

    println!("Created UEFI image: {}", image_path.display());

    Ok(())
}

fn get_binary_path(output: &[u8]) -> Result<Utf8PathBuf, anyhow::Error> {
    for message in Message::parse_stream(output) {
        if let Message::CompilerArtifact(artifact) = message? {
            if let Some(path) = artifact.executable {
                return Ok(path);
            }
        }
    }

    bail!("failed to extract binary path")
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

    Ok(())
}
