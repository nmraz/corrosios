use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use cargo_metadata::Message;
use xshell::{cmd, Cmd, Shell};

use crate::config;

pub fn cross_run_all(sh: &Shell, subcommand: &str, additional_args: &[String]) -> Result<()> {
    cross_run(
        sh,
        subcommand,
        config::KERNEL_PACKAGE_NAME,
        config::KERNEL_PACKAGE_TARGET,
        additional_args,
    )?;
    cross_run(
        sh,
        subcommand,
        config::BOOTLOADER_PACKAGE_NAME,
        config::BOOTLOADER_PACKAGE_TARGET,
        additional_args,
    )
}

pub fn kernel_binary_path(sh: &Shell, additional_args: &[String]) -> Result<PathBuf> {
    built_binary_path(
        sh,
        config::KERNEL_PACKAGE_NAME,
        config::KERNEL_PACKAGE_TARGET,
        additional_args,
    )
}

pub fn bootloader_binary_path(sh: &Shell, additional_args: &[String]) -> Result<PathBuf> {
    built_binary_path(
        sh,
        config::BOOTLOADER_PACKAGE_NAME,
        config::BOOTLOADER_PACKAGE_TARGET,
        additional_args,
    )
}

fn built_binary_path(
    sh: &Shell,
    package_name: &str,
    target: &str,
    additional_args: &[String],
) -> Result<PathBuf> {
    let cmd = freestanding_cross_cmd(sh, "build", package_name, target, additional_args)
        .arg("--message-format=json");

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
    sh: &Shell,
    subcommand: &str,
    package_name: &str,
    target: &str,
    additional_args: &[String],
) -> Result<()> {
    freestanding_cross_cmd(sh, subcommand, package_name, target, additional_args)
        .run()
        .with_context(|| format!("`cargo {subcommand}` failed"))
}

fn freestanding_cross_cmd<'a>(
    sh: &'a Shell,
    subcommand: &str,
    package_name: &str,
    target: &str,
    additional_args: &[String],
) -> Cmd<'a> {
    let cargo = env!("CARGO");

    cmd!(
        sh,
        "{cargo} {subcommand} -p {package_name} --target {target} -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem {additional_args...}"
    ).quiet()
}
