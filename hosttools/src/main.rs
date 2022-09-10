use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use hosttools::config;
use hosttools::cross::{cross_run_all, kernel_binary_path};
use hosttools::gdb::{run_gdb, GdbOptions};
use hosttools::image::{create_disk_image, ImageOptions};
use hosttools::qemu::{run_qemu, QemuOptions};
use xshell::{cmd, Shell};

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
    Gdbmux(GdbmuxSubcommand),
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
    #[clap(flatten)]
    args: ImageArgs,
}

#[derive(Args)]
struct ImageArgs {
    /// Build image in release mode
    #[clap(long)]
    release: bool,

    /// Additional arguments to use when building
    #[clap(short = 'B')]
    additional_build_args: Vec<String>,
}

/// Run UEFI image in QEMU.
#[derive(Args)]
struct QemuCommand {
    /// Enable GDB server in QEMU
    #[clap(long)]
    gdbserver: bool,

    /// Additional arguments to pass to QEMU
    additional_args: Vec<String>,

    #[clap(flatten)]
    common: QemuArgs,

    #[clap(flatten)]
    image: ImageArgs,
}

/// Run QEMU and GDB together in tmux.
#[derive(Args)]
struct GdbmuxSubcommand {
    #[clap(flatten)]
    qemu: QemuArgs,
}

#[derive(Args)]
struct QemuArgs {
    /// Enable KVM acceleration
    #[clap(long)]
    kvm: bool,

    /// Run in headless mode
    #[clap(long)]
    headless: bool,

    /// Serial value to pass to QEMU
    #[clap(long, default_value = "mon:stdio")]
    serial: String,
}

/// Run UEFI image in QEMU, attach gdb.
#[derive(Args)]
struct GdbAttachCommand {
    /// The address of the remote GDB server
    #[clap(long, default_value = "localhost:1234")]
    server: String,

    #[clap(flatten)]
    image: ImageArgs,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let sh = Shell::new()?;
    sh.change_dir(config::get_workspace_root()?);

    match &args.command {
        Command::Cross(cross) => cross_run_all(&sh, &cross.subcommand, &cross.additional_args),
        Command::Image(image) => {
            create_disk_image(&sh, &image_opts_from_image_args(&image.args))?;
            Ok(())
        }

        Command::Qemu(qemu) => {
            let image_path = create_disk_image(&sh, &image_opts_from_image_args(&qemu.image))?;

            let opts = QemuOptions {
                image_path: &image_path,
                enable_gdbserver: qemu.gdbserver,
                use_kvm: qemu.common.kvm,
                headless: qemu.common.headless,
                serial: &qemu.common.serial,
                additional_args: &qemu.additional_args,
            };

            run_qemu(&sh, &opts)
        }

        Command::GdbAttach(gdb) => {
            let image_opts = image_opts_from_image_args(&gdb.image);
            let kernel_path = kernel_binary_path(&sh, &image_opts.build_args())?;
            let gdb_opts = GdbOptions {
                kernel_binary: &kernel_path,
                server: &gdb.server,
            };

            run_gdb(&sh, &gdb_opts)
        }

        Command::Gdbmux(gdbmux) => {
            let image_opts = ImageOptions {
                release: false,
                additional_build_args: &[],
            };
            let image_path = create_disk_image(&sh, &image_opts)?;

            let qemu_opts = QemuOptions {
                image_path: &image_path,
                enable_gdbserver: true,
                use_kvm: gdbmux.qemu.kvm,
                headless: gdbmux.qemu.headless,
                serial: &gdbmux.qemu.serial,
                additional_args: &[],
            };

            let cargo = env!("CARGO");
            cmd!(sh, "tmux split-pane -h {cargo} gdb-attach")
                .quiet()
                .run()?;

            run_qemu(&sh, &qemu_opts)
        }
    }
}

fn image_opts_from_image_args(args: &ImageArgs) -> ImageOptions<'_> {
    ImageOptions {
        release: args.release,
        additional_build_args: &args.additional_build_args,
    }
}
