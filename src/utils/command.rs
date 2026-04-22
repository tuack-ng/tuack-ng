use crate::prelude::*;
use std::process::Command;

pub fn string_to_command(command_str: &str) -> Result<Command> {
    let parts = shellwords::split(command_str)?;

    if parts.is_empty() {
        bail!("Empty command");
    }

    let mut cmd = Command::new(&parts[0]);

    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    Ok(cmd)
}
