use crate::config::ContestDayConfig;
use crate::config::ExpectedScore;
use crate::config::ProblemConfig;
use crate::config::SampleItem;
use crate::config::ScorePolicy;
use crate::config::TestCase;
use crate::config::load_contest_config;
use crate::config::load_day_config;
use crate::config::load_problem_config;
use crate::config::save_contest_config;
use crate::config::save_day_config;
use crate::config::save_problem_config;
use crate::config::{ContestConfig, DataItem};
use crate::context::{CurrentLocation, get_context};
use crate::utils::optional::Optional;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use clap::Args;
use clap::Subcommand;
use clap_complete::Shell;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use log::error;
use log::warn;
use natord::compare;
use regex::Regex;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::{fs, path::PathBuf};

const CONFIG_FILE_NAME: &str = "conf.json";

#[derive(Debug, Clone, Subcommand)]
#[command(version)]
#[command(infer_subcommands = false)]
enum Targets {
    /// 生成竞赛文件夹
    #[command(version, aliases = ["n"])]
    Contest(GenStatementArgs),
    /// 生成竞赛日文件夹
    #[command(version, alias = "d")]
    Day(GenStatementArgs),
    /// 生成题目文件夹
    #[command(version, aliases = ["p", "prob"])]
    Problem(GenStatementArgs),

    /// 自动检测数据
    #[command(version, alias = "t")]
    Data(GenConfirmArgs),
    /// 自动检测样例
    #[command(version, alias = "s")]
    Samples(GenConfirmArgs),
    /// 自动检测题解
    #[command(version, alias = "c")]
    Code(GenConfirmArgs),
    /// 自动检测所有
    #[command(version, alias = "a")]
    All(GenConfirmArgs),

    /// 生成 Shell 补全文件
    #[command(version, hide = true)]
    Complete(GenCompleteArgs),
}

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct GenStatementArgs {
    /// 对象名称
    #[arg(required = true)]
    name: Vec<String>,
}

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct GenConfirmArgs {
    /// 跳过确认提示
    #[arg(short = 'y')]
    confirm: bool,
}

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct GenCompleteArgs {
    /// 对象名称
    #[arg(required = true)]
    name: String,
}

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct GenArgs {
    /// 生成的对象
    #[command(subcommand)]
    target: Targets,
}

fn gen_contest(args: GenStatementArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;

    // 查找scaffold/contest目录（在程序上下文中的列表中第一个存在的）
    let scaffold_path = find_scaffold_dir("contest")?;

    for contest_name in &args.name {
        copy_dir_recursive(&scaffold_path, current_dir.join(contest_name))?;

        let mut contest_json: ContestConfig =
            load_contest_config(&current_dir.join(contest_name).join(CONFIG_FILE_NAME))?;

        contest_json.name = contest_name.to_string();

        let updated_content = save_contest_config(&contest_json)?;
        std::fs::write(
            current_dir.join(contest_name).join(CONFIG_FILE_NAME),
            updated_content,
        )?;
    }
    Ok(())
}

fn gen_day(args: GenStatementArgs) -> Result<()> {
    // 检查是否在contest目录下执行
    let current_dir = std::env::current_dir()?;
    let config_path = current_dir.join(CONFIG_FILE_NAME);

    // 检查当前目录是否存在contest配置文件
    if !config_path.exists() {
        bail!("day命令必须在contest目录下执行");
    }

    // 检查配置文件是否为contest类型
    let content = std::fs::read_to_string(&config_path)?;
    let json_value: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(folder) = json_value.get("folder").and_then(|v| v.as_str()) {
        if folder != "contest" {
            bail!("day命令必须在contest目录下执行");
        }
    } else {
        bail!("无效的配置文件");
    }

    // 查找scaffold/day目录（在程序上下文中的列表中第一个存在的）
    let scaffold_path = find_scaffold_dir("day")?;

    for day_name in &args.name {
        copy_dir_recursive(&scaffold_path, current_dir.join(day_name))?;

        let mut day_json: ContestDayConfig =
            load_day_config(&current_dir.join(day_name).join(CONFIG_FILE_NAME))?;

        day_json.name = day_name.to_string();

        let updated_content = save_day_config(&day_json)?;
        std::fs::write(
            current_dir.join(day_name).join(CONFIG_FILE_NAME),
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
    Ok(())
}

fn gen_problem(args: GenStatementArgs) -> Result<()> {
    // 检查是否在day目录下执行
    let current_dir = std::env::current_dir()?;
    let config_path = current_dir.join(CONFIG_FILE_NAME);

    // 检查当前目录是否存在day配置文件
    if !config_path.exists() {
        bail!("problem命令必须在day目录下执行");
    }

    // 检查配置文件是否为day类型
    let content = std::fs::read_to_string(&config_path)?;
    let json_value: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(folder) = json_value.get("folder").and_then(|v| v.as_str()) {
        if folder != "day" {
            bail!("problem命令必须在day目录下执行");
        }
    } else {
        bail!("无效的配置文件");
    }

    // 查找scaffold/problem目录（在程序上下文中的列表中第一个存在的）
    let scaffold_path = find_scaffold_dir("problem")?;

    for problem_name in &args.name {
        copy_dir_recursive(&scaffold_path, current_dir.join(problem_name))?;

        let mut problem_json: ProblemConfig =
            load_problem_config(&current_dir.join(problem_name).join(CONFIG_FILE_NAME))?;

        problem_json.name = problem_name.to_string();

        let updated_content = save_problem_config(&problem_json)?;
        std::fs::write(
            current_dir.join(problem_name).join(CONFIG_FILE_NAME),
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
    Ok(())
}

fn confirm_overwrite() -> bool {
    let theme = ColorfulTheme::default();

    let items = vec!["No", "Yes"];

    let selection = Select::with_theme(&theme)
        .with_prompt(
            "警告：这个操作将不可逆转地覆盖您的数据点和/或子任务配置。
请谨慎运行，并且建议执行前备份您的配置文件。
是否确认继续（使用 -y 跳过提示）？",
        )
        .items(&items)
        .default(0)
        .interact()
        .unwrap();

    selection == 1
}

fn gen_data(args: GenConfirmArgs) -> Result<()> {
    if !args.confirm && !confirm_overwrite() {
        return Ok(());
    }

    let config = get_context().config.clone().context("没有有效的配置")?;
    for (now_day_name, day) in config.0.subconfig {
        let day_name: Option<String> = match config.1 {
            CurrentLocation::Day(ref name) => Some(name.clone()),
            CurrentLocation::Problem(ref day_name, _) => Some(day_name.clone()),
            _ => None,
        };
        if day_name.is_some() && now_day_name != day_name.unwrap() {
            continue;
        }
        for (now_problem_name, problem) in day.subconfig {
            let problem_name: Option<String> = match config.1 {
                CurrentLocation::Problem(_, ref name) => Some(name.clone()),
                _ => None,
            };
            if problem_name.is_some() && now_problem_name != problem_name.unwrap() {
                continue;
            }

            let data_dir = problem.path.join("data");
            if !data_dir.exists() {
                warn!("题目 {} 不存在 data 目录，跳过数据生成", problem.name);
                continue;
            }
            let mut datas_entrys = Vec::<String>::new();
            for entry in fs::read_dir(&data_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension()
                        && ext == "in"
                    {
                        let output_file = path.file_stem().unwrap().to_string_lossy() + ".ans";
                        let output_path = data_dir.join(output_file.as_ref());

                        if output_path.exists() {
                            datas_entrys
                                .push(path.file_stem().unwrap().to_string_lossy().to_string());
                        }
                    }
                }
            }
            datas_entrys.sort_by(|a, b| compare(a, b));
            let count = datas_entrys.len() as u32;

            let datas: Vec<DataItem> = datas_entrys
                .into_iter()
                .enumerate()
                .map(|(id, name)| DataItem {
                    id: id as u32 + 1,
                    input: Optional::initialized(format!("{}.in", name)),
                    output: Optional::initialized(format!("{}.ans", name)),
                    score: 100 / count,
                    subtest: 0,
                })
                .collect();
            let subtests: BTreeMap<u32, ScorePolicy> = BTreeMap::from([(0, ScorePolicy::Sum)]);

            let mut now_problem = load_problem_config(&problem.path.join(CONFIG_FILE_NAME))?;
            now_problem.data = datas;
            now_problem.subtests = subtests;

            let updated_content = save_problem_config(&now_problem)?;
            fs::write(problem.path.join(CONFIG_FILE_NAME), updated_content)?;
        }
    }

    Ok(())
}

fn gen_sample(args: GenConfirmArgs) -> Result<()> {
    if !args.confirm && !confirm_overwrite() {
        return Ok(());
    }

    let config = get_context().config.clone().context("没有有效的配置")?;
    for (now_day_name, day) in config.0.subconfig {
        let day_name: Option<String> = match config.1 {
            CurrentLocation::Day(ref name) => Some(name.clone()),
            CurrentLocation::Problem(ref day_name, _) => Some(day_name.clone()),
            _ => None,
        };
        if day_name.is_some() && now_day_name != day_name.unwrap() {
            continue;
        }
        for (now_problem_name, problem) in day.subconfig {
            let problem_name: Option<String> = match config.1 {
                CurrentLocation::Problem(_, ref name) => Some(name.clone()),
                _ => None,
            };
            if problem_name.is_some() && now_problem_name != problem_name.unwrap() {
                continue;
            }

            let data_dir = problem.path.join("sample");
            if !data_dir.exists() {
                warn!("题目 {} 不存在 sample 目录，跳过数据生成", problem.name);
                continue;
            }
            let mut datas_entrys = Vec::<String>::new();
            for entry in fs::read_dir(&data_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension()
                        && ext == "in"
                    {
                        let output_file = path.file_stem().unwrap().to_string_lossy() + ".ans";
                        let output_path = data_dir.join(output_file.as_ref());

                        if output_path.exists() {
                            datas_entrys
                                .push(path.file_stem().unwrap().to_string_lossy().to_string());
                        }
                    }
                }
            }
            datas_entrys.sort_by(|a, b| compare(a, b));

            let samples: Vec<SampleItem> = datas_entrys
                .into_iter()
                .enumerate()
                .map(|(id, name)| SampleItem {
                    id: id as u32 + 1,
                    input: Optional::initialized(format!("{}.in", name)),
                    output: Optional::initialized(format!("{}.ans", name)),
                })
                .collect();

            let mut now_problem = load_problem_config(&problem.path.join(CONFIG_FILE_NAME))?;
            now_problem.samples = samples;

            let updated_content = save_problem_config(&now_problem)?;
            fs::write(problem.path.join(CONFIG_FILE_NAME), updated_content)?;
        }
    }

    Ok(())
}

fn gen_code(args: GenConfirmArgs) -> Result<()> {
    let user_skip = Regex::new(r"^(data|down|pre|val|.*validate.*|gen|chk|checker|report|check.*|make_data|data_maker|data_make|make|dmk|generate|generator|makedata|spj|judge|tables|tmp|cp|copy|mv|move|rm|remove|.*\.tmp|.*\.temp|temp|.*\.test|.*\.dir)(\..*)?$").unwrap();
    fn find_code(path: &PathBuf, user_skip: &Regex) -> Result<Vec<(PathBuf, bool)>> {
        if user_skip.is_match(&path.to_string_lossy()) {
            return Ok(vec![]);
        }
        if path.is_dir() {
            let mut result = Vec::<(PathBuf, bool)>::new();
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();
                result.extend(find_code(&entry_path, user_skip)?);
            }
            return Ok(result);
        } else {
            for (key, _) in get_context().languages.iter() {
                if let Some(ext) = path.extension() {
                    if ext.to_string_lossy().as_ref() == key {
                        return Ok(vec![(
                            path.clone(),
                            path.file_stem().unwrap().to_string_lossy().as_ref() == "std",
                        )]);
                    }
                }
            }
            return Ok(vec![]);
        }
    }
    if !args.confirm && !confirm_overwrite() {
        return Ok(());
    }

    let config = get_context().config.clone().context("没有有效的配置")?;
    for (now_day_name, day) in config.0.subconfig {
        let day_name: Option<String> = match config.1 {
            CurrentLocation::Day(ref name) => Some(name.clone()),
            CurrentLocation::Problem(ref day_name, _) => Some(day_name.clone()),
            _ => None,
        };
        if day_name.is_some() && now_day_name != day_name.unwrap() {
            continue;
        }
        for (now_problem_name, problem) in day.subconfig {
            let problem_name: Option<String> = match config.1 {
                CurrentLocation::Problem(_, ref name) => Some(name.clone()),
                _ => None,
            };
            if problem_name.is_some() && now_problem_name != problem_name.unwrap() {
                continue;
            }

            let mut codes = find_code(&problem.path, &user_skip)?;
            if codes.is_empty() {
                warn!("题目 {} 不存在题解文件，跳过题解生成", problem.name);
                continue;
            }

            codes.sort_by(|a, b| {
                compare(
                    a.0.to_string_lossy().as_ref(),
                    b.0.to_string_lossy().as_ref(),
                )
            });

            let mut tests = HashMap::<String, TestCase>::new();

            for (path, is_std) in codes {
                let name = if is_std {
                    "std".to_string()
                } else {
                    // 相对路径作为名称
                    path.strip_prefix(&problem.path)
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                };
                let expected = if is_std {
                    ExpectedScore::Single("== 100".to_string())
                } else {
                    ExpectedScore::Multiple(vec![])
                };
                tests.insert(
                    name,
                    TestCase {
                        expected,
                        path: path
                            .strip_prefix(&problem.path)
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                    },
                );
            }

            let mut now_problem = load_problem_config(&problem.path.join(CONFIG_FILE_NAME))?;
            now_problem.tests = tests;

            let updated_content = save_problem_config(&now_problem)?;
            fs::write(problem.path.join(CONFIG_FILE_NAME), updated_content)?;
        }
    }

    Ok(())
}

fn gen_all(args: GenConfirmArgs) -> Result<()> {
    if !args.confirm && !confirm_overwrite() {
        return Ok(());
    }
    gen_data(GenConfirmArgs { confirm: true })?;
    gen_sample(GenConfirmArgs { confirm: true })?;
    gen_code(GenConfirmArgs { confirm: true })?;

    Ok(())
}

fn gen_complete(args: GenCompleteArgs) -> Result<()> {
    let shell = match args.name.to_lowercase().as_str() {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        _ => {
            error!("不支持的shell类型: {}", args.name);
            bail!("不支持的shell类型")
        }
    };
    let mut cmd = crate::Cli::command_i18n();
    clap_complete::generate(shell, &mut cmd, "tuack-ng", &mut io::stdout());

    Ok(())
}

pub fn main(args: GenArgs) -> Result<()> {
    match args.target {
        Targets::Contest(args) => gen_contest(args)?,
        Targets::Day(args) => gen_day(args)?,
        Targets::Problem(args) => gen_problem(args)?,
        Targets::Data(args) => gen_data(args)?,
        Targets::Samples(args) => gen_sample(args)?,
        Targets::All(args) => gen_all(args)?,
        Targets::Code(args) => gen_code(args)?,
        Targets::Complete(args) => gen_complete(args)?,
    }

    Ok(())
}

// 查找scaffold目录（在程序上下文中的列表中第一个存在的）
fn find_scaffold_dir(dir_name: &str) -> Result<PathBuf> {
    let context = crate::context::get_context();

    for assets_dir in &context.assets_dirs {
        let path = assets_dir.join("scaffold").join(dir_name);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(anyhow!("找不到scaffold/{}目录", dir_name))
}

// 递归复制目录的辅助函数
fn copy_dir_recursive<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<()> {
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
