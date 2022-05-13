use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use hosttools::config;
use hosttools::cross::{cross_run_all, kernel_binary_path};
use hosttools::gdb::{run_gdb, GdbOptions};
use hosttools::image::create_disk_image;
use hosttools::qemu::{run_qemu, QemuOptions};
use xshell::Shell;

/// Tools for use on the host.
#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Cross(CrossCommand),
    Image(ImageCommand),
    Qemu(QemuCommand),
    GdbAttach(GdbAttachCommand),
}

/// Run cargo subcommand with appropriate cross-compilation flags.
#[derive(Args)]
struct CrossCommand {
    subcommand: String,
    additional_args: Vec<String>,
}

/// Create a bootable UEFI image.
#[derive(Args)]
struct ImageCommand {
    /// Additional arguments to use when building
    additional_build_args: Vec<String>,
}

/// Run UEFI image in QEMU.
#[derive(Args)]
struct QemuCommand {
    /// Enable GDB server in QEMU
    #[clap(long)]
    gdbserver: bool,

    /// Run in headless mode
    #[clap(long)]
    headless: bool,

    /// Serial value to pass to QEMU
    #[clap(long, default_value = "mon:stdio")]
    serial: String,

    /// Additional arguments to use when building
    additional_build_args: Vec<String>,
}

/// Run UEFI image in QEMU, attach gdb.
#[derive(Args)]
struct GdbAttachCommand {
    /// The address of the remote GDB server
    #[clap(long, default_value = "localhost:1234")]
    server: String,

    /// Additional arguments to use when building the kernel to use with GDB
    additional_build_args: Vec<String>,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let sh = Shell::new()?;
    sh.change_dir(config::get_workspace_root()?);

    match &args.command {
        Command::Cross(cross) => cross_run_all(&sh, &cross.subcommand, &cross.additional_args),
        Command::Image(image) => {
            let image_path = create_disk_image(&sh, &image.additional_build_args)?;
            println!("Created UEFI image: {}", image_path.display());
            Ok(())
        }

        Command::Qemu(qemu) => {
            let image_path = create_disk_image(&sh, &qemu.additional_build_args)?;
            let opts = QemuOptions {
                image_path: &image_path,
                enable_gdbserver: qemu.gdbserver,
                serial: &qemu.serial,
                headless: qemu.headless,
            };

            run_qemu(&sh, &opts)
        }

        Command::GdbAttach(gdb) => {
            let kernel_path = kernel_binary_path(&sh, &gdb.additional_build_args)?;
            let gdb_opts = GdbOptions {
                kernel_binary: &kernel_path,
                server: &gdb.server,
            };

            run_gdb(&sh, &gdb_opts)
        }
    }
}
