# Corrosios

A WIP microkernel-based OS targeting modern x64 systems, written in Rust.

## Current status

The kernel currently contains a physical and virtual memory manager and is capable of displaying a blue screen when it boots.

## Quick start in QEMU

**Disclaimer:** I develop on an x64 Linux system, so I'm not really sure whether this project builds or runs on other platforms.

Install [rustup](https://rustup.rs) and QEMU:

```bash
# Fedora, etc. (should be installed by default in recent versions)
sudo dnf install qemu-kvm

# Debian/Ubuntu
sudo apt install qemu-system-x86
```

Boot up a basic image in QEMU/KVM with:

```bash
# Simple debug build, verbose logging
cargo qemu --kvm -k loglevel=debug

# Release build
cargo qemu --kvm --release
```

These will open a QEMU window with screen output and direct serial output to your terminal. You can also run a headless build or redirect serial output with the `--headless` and `--serial` flags; see the help message for more details.

**Notes:**

- Plain emulation (instead of KVM) should also work; run `cargo qemu` without `--kvm`.

- The first time this command is run, it will download the necessary toolchain and dependencies over the internet. Subsequent builds should run offline.

- If you'd rather not use rustup, make sure you are using the exact nightly Rust version specified in `rust-toolchain.toml`.

- Debugging support requires `gdb` to be installed as well.

## IDE Setup

See `.vscode/settings.defaults.json` for the settings I use for VSCode with rust-analyzer.

### Debugging

Source-level debugging support depends on the C/C++ extension. Once it is installed, you should be able to use the "Launch QEMU gdbserver" debug task.

## Cargo Subcommands and `hosttools`

This project uses a (currently pretty bloated, oops) Rust binary called `hosttools` to implement various custom cargo subcommands, in the spirit of the [xtask pattern](https://github.com/matklad/cargo-xtask).

Currently supported `cargo` subcommands:

- `hosttools` - Runs the hosttools binary itself, enabling direct access to all of its subcommands.
- `image` - Creates a UEFI-bootable GPT image.
- `qemu` - Creates an image and boots it in QEMU.
- `gdb-attach` - Attaches to a running QEMU machine with GDB.
- `gdb-split` (for Tilix users) - Boots a debug image in QEMU, and attaches GDB to the running image in a new pane.
- `cross` - Runs a cargo build subcommand (e.g., `check`, `clippy`, `build`, `doc`) across all sub-projects using the appropriate cross-compilation commands.
- `xbuild` - Shorthand for `cross build`.
- `xclippy` - Shorthand for `cross clippy`.
- `xclippy-json` - Like `xclippy`, but outputs diagnostics in json format (for use with rust-analyzer).
- `hclippy` - Runs `clippy` on the hosttools project itself.

## Directory Overview

- `hosttools/` - A binary implementing "scripts" and build utilities that run on the host system. It currently manages building the image and launching QEMU.
- `kernel/` - Kernel, (UEFI) bootloader and their support libraries.
  - `efiboot/` - UEFI bootloader capable of loading the kernel and its command line from the ESP.
  - `kernel/` - The kernel itself.
- `lib/` - General-purpose libraries usable across the kernel, userspace and hosttools.
- `qemu/` - QEMU-related configuration/data (currently contains a vendored OVMF image to avoid platform inconsistencies).
- `scripts/` - For things that have to be external scripts. Most "script-like" behavior should go into `hosttools` instead.
