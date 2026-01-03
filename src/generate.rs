use clap::Args;
use clap::ValueEnum;
use std::path::Path;
use std::{fs, path::PathBuf};

use crate::config::ContestConfig;
use crate::config::ContestDayConfig;
use crate::config::ProblemConfig;
use crate::config::load_contest_config;
use crate::config::load_day_config;
use crate::config::load_problem_config;
use crate::config::save_contest_config;
use crate::config::save_day_config;
use crate::config::save_problem_config;

const CONFIG_FILE_NAME: &str = "conf.json";

#[derive(Debug, Clone, ValueEnum)]
enum Targets {
    Contest,
    Day,
    Problem,
}

#[derive(Args, Debug)]
#[command(version)]
pub struct GenArgs {
    /// 生成的对象
    #[arg(required = true, value_enum)]
    target: Targets,

    /// 对象名称
    #[arg(required = true)]
    name: Vec<String>,
}

pub fn main(args: GenArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.target {
        Targets::Contest => {
            let current_dir = std::env::current_dir()?;

            // 查找scaffold/contest目录（在程序上下文中的列表中第一个存在的）
            let scaffold_path = find_scaffold_dir("contest")?;

            for contest_name in &args.name {
                copy_dir_recursive(&scaffold_path, &current_dir.join(contest_name))?;

                let mut contest_json: ContestConfig =
                    load_contest_config(&current_dir.join(contest_name).join(CONFIG_FILE_NAME))?;

                contest_json.name = contest_name.to_string();

                let updated_content = save_contest_config(&contest_json)?;
                std::fs::write(
                    &current_dir.join(contest_name).join(CONFIG_FILE_NAME),
                    updated_content,
                )?;
            }
        }
        Targets::Day => {
            // 检查是否在contest目录下执行
            let current_dir = std::env::current_dir()?;
            let config_path = current_dir.join(CONFIG_FILE_NAME);

            // 检查当前目录是否存在contest配置文件
            if !config_path.exists() {
                return Err("day命令必须在contest目录下执行".into());
            }

            // 检查配置文件是否为contest类型
            let content = std::fs::read_to_string(&config_path)?;
            let json_value: serde_json::Value = serde_json::from_str(&content)?;

            if let Some(folder) = json_value.get("folder").and_then(|v| v.as_str()) {
                if folder != "contest" {
                    return Err("day命令必须在contest目录下执行".into());
                }
            } else {
                return Err("无效的配置文件".into());
            }

            // 查找scaffold/day目录（在程序上下文中的列表中第一个存在的）
            let scaffold_path = find_scaffold_dir("day")?;

            for day_name in &args.name {
                copy_dir_recursive(&scaffold_path, &current_dir.join(day_name))?;

                let mut day_json: ContestDayConfig =
                    load_day_config(&current_dir.join(day_name).join(CONFIG_FILE_NAME))?;

                day_json.name = day_name.to_string();

                let updated_content = save_day_config(&day_json)?;
                std::fs::write(
                    &current_dir.join(day_name).join(CONFIG_FILE_NAME),
                    updated_content,
                )?;
            }

            // 更新contest配置文件的subdir字段
            let mut contest_config: serde_json::Value = serde_json::from_str(&content)?;
            if let Some(subdir) = contest_config
                .get_mut("subdir")
                .and_then(|v| v.as_array_mut())
            {
                for day_name in &args.name {
                    subdir.push(serde_json::Value::String(day_name.clone()));
                }
            }

            let updated_content = serde_json::to_string_pretty(&contest_config)?;
            std::fs::write(&config_path, updated_content)?;
        }
        Targets::Problem => {
            // 检查是否在day目录下执行
            let current_dir = std::env::current_dir()?;
            let config_path = current_dir.join(CONFIG_FILE_NAME);

            // 检查当前目录是否存在day配置文件
            if !config_path.exists() {
                return Err("problem命令必须在day目录下执行".into());
            }

            // 检查配置文件是否为day类型
            let content = std::fs::read_to_string(&config_path)?;
            let json_value: serde_json::Value = serde_json::from_str(&content)?;

            if let Some(folder) = json_value.get("folder").and_then(|v| v.as_str()) {
                if folder != "day" {
                    return Err("problem命令必须在day目录下执行".into());
                }
            } else {
                return Err("无效的配置文件".into());
            }

            // 查找scaffold/problem目录（在程序上下文中的列表中第一个存在的）
            let scaffold_path = find_scaffold_dir("problem")?;

            for problem_name in &args.name {
                copy_dir_recursive(&scaffold_path, &current_dir.join(problem_name))?;

                let mut problem_json: ProblemConfig =
                    load_problem_config(&current_dir.join(problem_name).join(CONFIG_FILE_NAME))?;

                problem_json.name = problem_name.to_string();

                let updated_content = save_problem_config(&problem_json)?;
                std::fs::write(
                    &current_dir.join(problem_name).join(CONFIG_FILE_NAME),
                    updated_content,
                )?;
            }

            // 更新day配置文件的subdir字段
            let mut day_config: serde_json::Value = serde_json::from_str(&content)?;
            if let Some(subdir) = day_config.get_mut("subdir").and_then(|v| v.as_array_mut()) {
                for problem_name in &args.name {
                    subdir.push(serde_json::Value::String(problem_name.clone()));
                }
            }

            let updated_content = serde_json::to_string_pretty(&day_config)?;
            std::fs::write(&config_path, updated_content)?;
        }
    }

    Ok(())
}

// 查找scaffold目录（在程序上下文中的列表中第一个存在的）
fn find_scaffold_dir(dir_name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let context = crate::context::get_context();

    for scaffold_dir in &context.scaffold_dirs {
        let path = scaffold_dir.join(dir_name);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(format!("找不到scaffold/{}目录", dir_name).into())
}

// 递归复制目录的辅助函数
fn copy_dir_recursive<P: AsRef<Path>, Q: AsRef<Path>>(
    src: P,
    dst: Q,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}