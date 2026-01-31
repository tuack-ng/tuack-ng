use crate::config::lang::Language;
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

pub fn build_compile_cmd(
    day_config: &ContestDayConfig,
    problem_config: &ProblemConfig,
    target_path: &PathBuf,
    ext: String,
    file_type: &Language,
) -> Result<Command, anyhow::Error> {
    Ok(string_to_command(
        format!(
            " {} {} {} {} {}",
            file_type.compiler.executable,
            file_type.compiler.object_set_arg,
            &problem_config.name,
            &day_config.compile.get(&ext).context("未知格式文件")?,
            target_path.file_name().unwrap().to_string_lossy()
        )
        .as_str(),
    )?)
}
