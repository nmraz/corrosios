use std::fs::OpenOptions;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use argh::FromArgs;
use fscommon::StreamSlice;
use hosttools::{
    add_efi_partition, built_binary_path, cargo_build_freestanding, format_efi_partition,
    format_gpt, DISK_SIZE,
};

const EFI_PACKAGE_NAME: &str = "efiapp";
const EFI_PACKAGE_TARGET: &str = "x86_64-unknown-uefi";

#[derive(FromArgs)]
/// Tools for use on the host.
struct Args {
    #[argh(subcommand)]
    subcommand: Subcommand,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Subcommand {
    Build(BuildSubcommand),
    Image(ImageSubcommand),
}

#[derive(FromArgs)]
/// Build binary with appropriate cargo flags.
#[argh(subcommand, name = "build")]
struct BuildSubcommand {
    #[argh(positional)]
    subcommand: String,

    #[argh(positional)]
    additional_args: Vec<String>,
}

#[derive(FromArgs)]
/// Create a bootable UEFI image.
#[argh(subcommand, name = "image")]
struct ImageSubcommand {
    #[argh(positional)]
    additional_args: Vec<String>,
}

fn main() -> Result<()> {
    let args: Args = argh::from_env();

    match &args.subcommand {
        Subcommand::Build(build) => build_efi_app(&build.subcommand, &build.additional_args),
        Subcommand::Image(image) => {
            let image_path = create_uefi_image(&image.additional_args)?;
            println!("Created UEFI image: {}", image_path.display());
            Ok(())
        }
    }
}

fn create_uefi_image(additional_args: &[String]) -> Result<PathBuf> {
    build_efi_app("build", additional_args)?;
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

fn build_efi_app(subcommand: &str, additional_args: &[String]) -> Result<()> {
    let status = cargo_build_freestanding(
        subcommand,
        EFI_PACKAGE_NAME,
        EFI_PACKAGE_TARGET,
        additional_args,
    )?;
    if !status.success() {
        bail!("`cargo {}` failed", subcommand);
    }
    Ok(())
}

fn efi_binary_path(additional_args: &[String]) -> Result<PathBuf> {
    built_binary_path(EFI_PACKAGE_NAME, EFI_PACKAGE_TARGET, additional_args)
}
