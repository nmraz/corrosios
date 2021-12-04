use anyhow::Result;
use argh::FromArgs;

use hosttools::cross::cross_run_all;
use hosttools::image::create_disk_image;
use hosttools::qemu::run_qemu;

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
    /// enable GDB server in QEMU
    #[argh(switch)]
    gdbserver: bool,

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
            let mut args = vec![];

            if qemu.gdbserver {
                args.extend(["-s".to_owned(), "-S".to_owned()]);
            }

            run_qemu(&image_path, &args)
        }
    }
}
