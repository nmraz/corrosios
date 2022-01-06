use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use fatfs::{FileSystem, FormatVolumeOptions, FsOptions, ReadWriteSeek};
use fscommon::StreamSlice;
use gpt::disk::LogicalBlockSize;
use gpt::mbr::ProtectiveMBR;
use gpt::{GptConfig, GptDisk};

use crate::config;
use crate::cross::{bootloader_binary_path, cross_run_all, kernel_binary_path};

const KB: u64 = 1024;
const MB: u64 = KB * KB;

const LB_SIZE: u64 = 512;

const EFI_PARTITION_SIZE: u64 = 10 * MB;
const DISK_SIZE: u64 = EFI_PARTITION_SIZE + 64 * KB;

pub fn create_disk_image(build_args: &[String]) -> Result<PathBuf> {
    cross_run_all("build", build_args)?;

    let kernel_path = kernel_binary_path(build_args)?;
    let bootloader_path = bootloader_binary_path(build_args)?;

    let image_path = bootloader_path.with_file_name(config::IMAGE_NAME);

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
    format_efi_partition(efi_part_data, &kernel_path, &bootloader_path)
        .context("failed to write EFI system partition")?;

    Ok(image_path)
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

fn format_efi_partition(
    mut partition: impl ReadWriteSeek,
    kernel_path: &Path,
    bootloader_path: &Path,
) -> Result<()> {
    fatfs::format_volume(&mut partition, FormatVolumeOptions::new())?;
    let fs = FileSystem::new(partition, FsOptions::new())?;
    let root = fs.root_dir();

    let mut kernel_file = root.create_dir("corrosios")?.create_file("kernel")?;
    io::copy(&mut File::open(kernel_path)?, &mut kernel_file)?;

    let mut boot_file = root
        .create_dir("efi")?
        .create_dir("boot")?
        .create_file("bootx64.efi")?;
    io::copy(&mut File::open(bootloader_path)?, &mut boot_file)?;

    Ok(())
}
