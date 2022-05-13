use anyhow::Result;
use argh::FromArgs;

use hosttools::cross::{cross_run_all, kernel_binary_path};
use hosttools::gdb::{run_gdb, GdbOptions};
use hosttools::image::create_disk_image;
use hosttools::qemu::{run_qemu, QemuOptions};

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
    Gdb(GdbSubcommand),
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

    /// run in headless mode
    #[argh(switch)]
    headless: bool,

    /// serial value to pass to QEMU
    #[argh(option, default = "String::from(\"mon:stdio\")")]
    serial: String,

    #[argh(positional)]
    additional_build_args: Vec<String>,
}

#[derive(FromArgs)]
/// Run UEFI image in QEMU, attach gdb.
#[argh(subcommand, name = "gdb")]
struct GdbSubcommand {
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
            let opts = QemuOptions {
                image_path: &image_path,
                enable_gdbserver: qemu.gdbserver,
                serial: &qemu.serial,
                headless: qemu.headless,
            };

            run_qemu(&opts)?.wait()
        }

        Subcommand::Gdb(gdb) => {
            let image_path = create_disk_image(&gdb.additional_build_args)?;
            let qemu_opts = QemuOptions {
                image_path: &image_path,
                enable_gdbserver: true,
                serial: "",
                headless: false,
            };

            let kernel_path = kernel_binary_path(&gdb.additional_build_args)?;
            let gdb_opts = GdbOptions {
                kernel_binary: &kernel_path,
                server_port: 1234,
            };

            let qemu_child = run_qemu(&qemu_opts)?;
            let mut gdb_child = run_gdb(&gdb_opts)?;

            gdb_child.wait()?;
            qemu_child.wait()?;

            Ok(())
        }
    }
}
