use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Result};
use cargo_metadata::Message;

use crate::config;

pub fn cross_run_all(subcommand: &str, additional_args: &[String]) -> Result<()> {
    cross_run(
        subcommand,
        config::KERNEL_PACKAGE_NAME,
        config::KERNEL_PACKAGE_TARGET,
        additional_args,
    )?;
    cross_run(
        subcommand,
        config::BOOTLOADER_PACKAGE_NAME,
        config::BOOTLOADER_PACKAGE_TARGET,
        additional_args,
    )
}

pub fn kernel_binary_path(additional_args: &[String]) -> Result<PathBuf> {
    built_binary_path(
        config::KERNEL_PACKAGE_NAME,
        config::KERNEL_PACKAGE_TARGET,
        additional_args,
    )
}

pub fn bootloader_binary_path(additional_args: &[String]) -> Result<PathBuf> {
    built_binary_path(
        config::BOOTLOADER_PACKAGE_NAME,
        config::BOOTLOADER_PACKAGE_TARGET,
        additional_args,
    )
}

fn built_binary_path(
    package_name: &str,
    target: &str,
    additional_args: &[String],
) -> Result<PathBuf> {
    let mut cmd = freestanding_cross_cmd("build", package_name, target, additional_args);
    cmd.arg("--message-format=json");

    let output = cmd.output()?.stdout;

    for message in Message::parse_stream(&output[..]) {
        if let Message::CompilerArtifact(artifact) = message? {
            if let Some(path) = artifact.executable {
                return Ok(path.into());
            }
        }
    }

    bail!("failed to extract binary path")
}

fn cross_run(
    subcommand: &str,
    package_name: &str,
    target: &str,
    additional_args: &[String],
) -> Result<()> {
    let mut cmd = freestanding_cross_cmd(subcommand, package_name, target, additional_args);
    if !cmd.status()?.success() {
        bail!("`cargo {}` failed", subcommand);
    }
    Ok(())
}

fn freestanding_cross_cmd(
    subcommand: &str,
    package_name: &str,
    target: &str,
    additional_args: &[String],
) -> Command {
    let mut cmd = Command::new(env!("CARGO"));
    cmd.args([
        subcommand,
        "-p",
        package_name,
        "--target",
        target,
        "-Zbuild-std=core,alloc",
        "-Zbuild-std-features=compiler-builtins-mem",
    ]);
    cmd.args(additional_args);

    cmd
}
