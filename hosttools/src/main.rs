use std::process::Command;

use anyhow::{Context, Result};
use argh::FromArgs;

use hosttools::{create_disk_image, cross_run_all};

#[derive(FromArgs)]
/// Tools for use on the host.
struct Args {
    #[argh(subcommand)]
    subcommand: Subcommand,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Subcommand {
    Cross(CrossSubcommand),
    Image(ImageSubcommand),
    Qemu(QemuSubcommand),
}

#[derive(FromArgs)]
/// Run cargo subcommand with appropriate cross-compilation flags.
#[argh(subcommand, name = "cross")]
struct CrossSubcommand {
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
    additional_build_args: Vec<String>,
}

#[derive(FromArgs)]
/// Run UEFI image in QEMU.
#[argh(subcommand, name = "qemu")]
struct QemuSubcommand {
    #[argh(option)]
    /// path to bios to give QEMU
    firmware_path: String,

    #[argh(positional)]
    additional_build_args: Vec<String>,
}

fn main() -> Result<()> {
    let args: Args = argh::from_env();

    match &args.subcommand {
        Subcommand::Cross(cross) => cross_run_all(&cross.subcommand, &cross.additional_args),
        Subcommand::Image(image) => {
            let image_path = create_disk_image(&image.additional_build_args)?;
            println!("Created UEFI image: {}", image_path.display());
            Ok(())
        }
        Subcommand::Qemu(qemu) => {
            let image_path = create_disk_image(&qemu.additional_build_args)?;
            let mut cmd = Command::new("qemu-system-x86_64");
            cmd.args([
                "-bios",
                &qemu.firmware_path,
                "-drive",
                &format!("file={},format=raw", image_path.display()),
            ]);
            cmd.spawn().context("failed to start QEMU")?.wait()?;
            Ok(())
        }
    }
}
