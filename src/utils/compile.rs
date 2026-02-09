use crate::config::lang::Language;
use crate::prelude::*;
use std::process::Command;
use strfmt::strfmt;

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
    if let Some(ref compiler) = language.compiler {
        debug!("检查 {} 编译环境", language.language);
        let check_cmd = strfmt(
            &compiler.check,
            &HashMap::from([("executable".to_string(), compiler.executable.clone())]),
        )?;
        match string_to_command(&check_cmd)?.output() {
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
    }

    Ok(())
}

pub fn check_runner(language: &Language) -> Result<()> {
    if let Some(ref runner) = language.runner {
        debug!("检查 {} 运行环境", language.language);
        let check_cmd = strfmt(
            &runner.check,
            &HashMap::from([("executable".to_string(), runner.executable.clone())]),
        )?;
        match string_to_command(&check_cmd)?.output() {
            Ok(output) if output.status.success() => {
                let version_output = String::from_utf8_lossy(&output.stdout);
                let version = version_output.lines().next().unwrap_or("").trim();
                debug!("{} 版本: {}", &runner.executable, version);
            }
            _ => {
                error!(
                    "未找到 {} 命令，请确保已安装并添加到PATH",
                    &runner.executable
                );
                bail!("{} 命令执行失败", &runner.executable);
            }
        }
    }

    Ok(())
}

// 可用变量：
// {executable}：同 executable
// {output_path}: 输出目录
// {program_name}：这道题叫啥（也是预期文件名）
// {args}：用户自定义文件名
// {input_path}：源文件路径
// {exe_suffix}：exe后缀名
pub fn build_compile_cmd(
    src_path: &Path,
    target_path: &Path,
    program_name: &str,
    compile_args: &HashMap<String, String>,
) -> Result<Option<Command>> {
    let ext = src_path
        .extension()
        .context("文件无后缀名")?
        .to_string_lossy();

    let file_type = get_context()
        .languages
        .get(ext.as_ref())
        .context("未知格式文件")?;

    check_compiler(file_type)?;

    if let Some(ref compile) = file_type.compiler {
        let compile_cmd = strfmt(
            &compile.run,
            &HashMap::from([
                ("executable".to_string(), compile.executable.clone()),
                (
                    "output_path".to_string(),
                    target_path.to_string_lossy().to_string(),
                ),
                ("program_name".to_string(), program_name.to_owned()),
                (
                    "args".to_string(),
                    compile_args
                        .get(ext.as_ref())
                        .context("没有该语言编译选项")?
                        .to_string(),
                ),
                (
                    "input_path".to_string(),
                    src_path.to_string_lossy().to_string(),
                ),
                (
                    "exe_suffix".to_string(),
                    std::env::consts::EXE_SUFFIX.to_string(),
                ),
            ]),
        )?;
        Ok(Some(string_to_command(compile_cmd.as_str())?))
    } else {
        Ok(None)
    }
}

/// `runner` 可用变量：
///
/// - `{executable}`：同 executable
/// - `{input_path}`：编译器产物路径（上一步的output_path）如果跳过编译会直接拷贝源文件（保留后缀但名字变成这道题）
/// - `{program_name}`：这道题叫啥（也是预期文件名）
/// - `{exe_suffix}`：exe后缀名
pub fn build_run_cmd(
    src_path: &Path,
    target_path: &Path,
    program_name: &str,
) -> Result<Option<Command>> {
    let ext = src_path
        .extension()
        .context("文件无后缀名")?
        .to_string_lossy();

    let file_type = get_context()
        .languages
        .get(ext.as_ref())
        .context("未知格式文件")?;

    check_runner(file_type)?;

    if let Some(ref runner) = file_type.runner {
        let compile_cmd = strfmt(
            &runner.run,
            &HashMap::from([
                ("executable".to_string(), runner.executable.clone()),
                (
                    "input_path".to_string(),
                    target_path.to_string_lossy().to_string(),
                ),
                ("program_name".to_string(), program_name.to_owned()),
                (
                    "exe_suffix".to_string(),
                    std::env::consts::EXE_SUFFIX.to_string(),
                ),
            ]),
        )?;
        Ok(Some(string_to_command(compile_cmd.as_str())?))
    } else {
        Ok(None)
    }
}
