use crate::tuack_lib::doc::rules::*;
use crate::{config::CONFIG_FILE_NAME, doc::rules::*, prelude::*};
use clap::Args;
use serde::{Deserialize, Serialize};
use tuack_ng_parser::parse;

#[derive(Args, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[command(version)]
pub struct FormatArgs {
    /// 解释这个规则
    #[arg(long)]
    explain: Option<String>,
}

fn get_formatters() -> Vec<Box<dyn FormatRule>> {
    vec![
        Box::new(invisible::Invisible),
        Box::new(samples_should_be_external::SamplesShouldBeExternal),
        Box::new(samples_too_large::SamplesTooLarge),
        Box::new(autocorrect::Autocorrect),
    ]
}

pub fn format(problem_config: &ProblemConfig) -> Result<()> {
    let markdown_path = problem_config.path.join("statement.md");
    let markdown_backup_path = markdown_path.with_extension("md.bak");
    fs::copy(&markdown_path, &markdown_backup_path)?;

    let mut markdown_text = fs::read_to_string(&markdown_path)?;

    let formatters = get_formatters();
    let mut problem_config = problem_config.to_owned();

    for formatter in &formatters {
        // 每个规则先应用文本规则，再解析为 AST 应用 AST 规则
        if formatter.manifest().markdown_formatter {
            debug!("正在应用文本格式化规则 {}", formatter.manifest().name);
            (markdown_text, problem_config) =
                formatter.apply_markdown(markdown_text, problem_config)?;
        }

        let mut ast = parse(&markdown_text);

        if formatter.manifest().ast_formatter {
            debug!("正在应用格式化规则 {}", formatter.manifest().name);
            (ast, problem_config) = formatter.apply_ast(ast, problem_config)?;
        }

        // 渲染回 Markdown，供下一轮规则使用
        markdown_text = tuack_ng_parser::printers::render_markdown(&ast);
    }

    fs::write(&markdown_path, markdown_text)?;

    let problem_config_text = problem_config.save()?;

    fs::write(
        problem_config.path.join(CONFIG_FILE_NAME),
        problem_config_text,
    )?;

    Ok(())
}

pub fn format_day(day_config: &ContestDayConfig) -> Result<()> {
    for (_, problem_config) in &day_config.subconfig {
        format(problem_config)?;
    }
    Ok(())
}

fn explain(id: String) -> Result<()> {
    let formatters = get_formatters();

    for formatter in formatters {
        if formatter.manifest().name == id {
            msg!("规则 {}: {}", id, formatter.manifest().description);
            return Ok(());
        }
    }

    bail!("找不到规则 {}", id);
}

pub fn main(args: FormatArgs) -> Result<()> {
    if let Some(rule) = args.explain {
        explain(rule)?;
        return Ok(());
    }

    let config = gctx().config.as_ref().context("没有可用的工程")?;

    match &config.location {
        CurrentLocation::None => bail!("没有可用的工程"),
        CurrentLocation::Root => {
            for (_, day_config) in &config.config.subconfig {
                format_day(day_config)?;
            }
        }
        CurrentLocation::Day(day) => {
            format_day(config.config.subconfig.get(day).unwrap())?;
        }
        CurrentLocation::Problem(day, problem) => {
            format(
                config
                    .config
                    .subconfig
                    .get(day)
                    .unwrap()
                    .subconfig
                    .get(problem)
                    .unwrap(),
            )?;
        }
    }

    Ok(())
}
