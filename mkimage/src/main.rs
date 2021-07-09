use std::process::Command;

use anyhow::{bail, Result};
use cargo_metadata::{camino::Utf8PathBuf, Message};

fn main() -> Result<()> {
    let mut cargo_cmd = Command::new(env!("CARGO"));
    cargo_cmd.args([
        "build",
        "-p",
        "efiapp",
        "--target",
        "x86_64-unknown-uefi",
        "-Zbuild-std=core",
        "-Zbuild-std-features=compiler-builtins-mem",
    ]);

    if !cargo_cmd.status()?.success() {
        bail!("Failed to build");
    }

    cargo_cmd.arg("--message-format=json");

    let exe = get_executable(&cargo_cmd.output()?.stdout)?;
    println!("Executable: {}", exe);

    Ok(())
}

fn get_executable(output: &[u8]) -> Result<Utf8PathBuf, anyhow::Error> {
    for message in Message::parse_stream(output) {
        if let Message::CompilerArtifact(artifact) = message? {
            if let Some(path) = artifact.executable {
                return Ok(path);
            }
        }
    }

    bail!("Failed to extract executable path")
}
