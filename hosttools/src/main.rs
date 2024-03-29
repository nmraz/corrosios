use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use hosttools::config;
use hosttools::cross::{cross_run_all, kernel_binary_path};
use hosttools::gdb::{run_gdb, GdbOptions};
use hosttools::image::{create_disk_image, ImageBuildOptions};
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
    GdbSplit(GdbSplitSubcommand),
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
struct BuildArgs {
    /// Build image in release mode
    #[clap(long)]
    release: bool,

    /// Additional arguments to use when building
    #[clap(short = 'B', allow_hyphen_values = true)]
    additional_build_args: Vec<String>,
}

#[derive(Args)]
struct ImageArgs {
    /// Arguments to add to the kernel command line
    #[clap(short = 'k', long = "kernel-arg")]
    kernel_command_line: Vec<String>,

    #[clap(flatten)]
    build: BuildArgs,
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

/// Run QEMU and GDB together in Tilix.
#[derive(Args)]
struct GdbSplitSubcommand {
    /// Arguments to add to the kernel command line
    #[clap(short = 'k', long = "kernel-arg")]
    kernel_command_line: Vec<String>,

    /// Build image in release mode
    #[clap(long)]
    release: bool,

    #[clap(flatten)]
    qemu: QemuArgs,
}

#[derive(Args)]
struct QemuArgs {
    /// Amount of memory to give guest
    #[clap(short = 'm', long = "mem", default_value = "1G")]
    mem: String,

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

/// Attach GDB to a running QEMU instance.
#[derive(Args)]
struct GdbAttachCommand {
    /// The address of the remote GDB server
    #[clap(long, default_value = "localhost:1234")]
    server: String,

    #[clap(flatten)]
    build: BuildArgs,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let sh = Shell::new()?;
    sh.change_dir(config::get_workspace_root()?);

    match &args.command {
        Command::Cross(cross) => cross_run_all(&sh, &cross.subcommand, &cross.additional_args),
        Command::Image(image) => {
            create_disk_image_from_args(&sh, &image.args)?;
            Ok(())
        }

        Command::Qemu(qemu) => {
            let image_path = create_disk_image_from_args(&sh, &qemu.image)?;

            let opts = QemuOptions {
                image_path: &image_path,
                mem: &qemu.common.mem,
                enable_gdbserver: qemu.gdbserver,
                use_kvm: qemu.common.kvm,
                headless: qemu.common.headless,
                serial: &qemu.common.serial,
                additional_args: &qemu.additional_args,
            };

            run_qemu(&sh, &opts)
        }

        Command::GdbAttach(gdb) => {
            let build_opts = build_opts_from_build_args(&gdb.build);
            let kernel_path = kernel_binary_path(&sh, &build_opts.build_args())?;
            let gdb_opts = GdbOptions {
                kernel_binary: &kernel_path,
                server: &gdb.server,
            };

            run_gdb(&sh, &gdb_opts)
        }

        Command::GdbSplit(gdb_split) => {
            let image_opts = ImageBuildOptions {
                release: gdb_split.release,
                additional_build_args: &[],
            };

            let image_path = create_disk_image(
                &sh,
                &image_opts,
                &kernel_command_line_from_args(&gdb_split.kernel_command_line),
            )?;

            let qemu_opts = QemuOptions {
                image_path: &image_path,
                mem: &gdb_split.qemu.mem,
                enable_gdbserver: true,
                use_kvm: gdb_split.qemu.kvm,
                headless: gdb_split.qemu.headless,
                serial: &gdb_split.qemu.serial,
                additional_args: &[],
            };

            let cargo = env!("CARGO");
            let mut attach_command = format!("{cargo} gdb-attach");
            if gdb_split.release {
                attach_command += " --release";
            }

            cmd!(sh, "tilix -a session-add-auto -x {attach_command}")
                .quiet()
                .run()?;

            run_qemu(&sh, &qemu_opts)
        }
    }
}

fn create_disk_image_from_args(sh: &Shell, args: &ImageArgs) -> Result<PathBuf> {
    let build_opts = build_opts_from_build_args(&args.build);
    let kernel_command_line = kernel_command_line_from_args(&args.kernel_command_line);
    create_disk_image(sh, &build_opts, &kernel_command_line)
}

const DEFAULT_KERNEL_COMMAND_LINE: &[u8] = b"x86.serial=3f8";

fn kernel_command_line_from_args(args: &[String]) -> Vec<u8> {
    let mut kernel_command_line = DEFAULT_KERNEL_COMMAND_LINE.to_owned();

    for arg in args {
        kernel_command_line.extend(b" ");
        kernel_command_line.extend(arg.as_bytes());
    }

    kernel_command_line
}

fn build_opts_from_build_args(args: &BuildArgs) -> ImageBuildOptions<'_> {
    ImageBuildOptions {
        release: args.release,
        additional_build_args: &args.additional_build_args,
    }
}
