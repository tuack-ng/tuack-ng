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

pub fn check_compiler(language: &Language) -> Result<()> {
    // 检查编译环境
    let compiler = &language.compiler;
    debug!("检查 {} 环境", language.language);
    match Command::new(&compiler.executable)
        .arg(&compiler.version_check)
        .output()
    {
        Ok(output) if output.status.success() => {
            let version_output = String::from_utf8_lossy(&output.stdout);
            let version = version_output.lines().next().unwrap_or("").trim();
            debug!("{} 版本: {}", &compiler.executable, version);
        }
        _ => {
            error!(
                "未找到 {} 命令，请确保已安装并添加到PATH",
                &compiler.executable
            );
            bail!("{} 命令执行失败", &compiler.executable);
        }
    }
    Ok(())
}

pub fn build_compile_cmd(
    src_path: &PathBuf,
    target_path: &PathBuf,
    compile_args: &HashMap<String, String>,
) -> Result<Command, anyhow::Error> {
    let ext = src_path
        .extension()
        .context("文件无后缀名")?
        .to_string_lossy();

    let file_type = get_context()
        .languages
        .get(ext.as_ref())
        .context("未知格式文件")?;

    check_compiler(file_type)?;

    Ok(string_to_command(
        format!(
            " {} {} {} {} {}",
            file_type.compiler.executable,
            file_type.compiler.object_set_arg,
            target_path.to_string_lossy(),
            compile_args.get(&ext.to_string()).context("未知格式文件")?,
            src_path.to_string_lossy()
        )
        .as_str(),
    )?)
}
