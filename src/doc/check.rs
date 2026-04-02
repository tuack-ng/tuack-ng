use crate::{doc::rules::*, prelude::*};
use clap::Args;
use markdown_ppp::parser::*;

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct CheckArgs {
    /// 解释这个规则
    #[arg(long)]
    explain: Option<String>,
}

fn get_checkers() -> Vec<Box<dyn CheckRule>> {
    vec![
        Box::new(invisible::Invisible),
        Box::new(samples_should_be_external::SamplesShouldBeExternal),
        Box::new(samples_too_large::SamplesTooLarge),
        Box::new(samples_not_found::SamplesNotFound),
        Box::new(autocorrect::Autocorrect),
        Box::new(latex::Latex),
        Box::new(html::Html),
    ]
}

fn print_messages(messages: CheckResult, path: &Path, checker: &dyn CheckRule) {
    match messages {
        CheckResult::Untagged(num) => {
            if num > 0 {
                msg_warn!(
                    "{} 检查器在文件 {} 中检测到 {} 个问题。使用 `doc format` 来修复",
                    checker.manifest().name.green(),
                    format!(
                        "{}",
                        path.strip_prefix(&gctx().config.as_ref().unwrap().0.path)
                            .unwrap()
                            .display()
                    )
                    .cyan(),
                    num
                );
            }
        }
        CheckResult::Tagged(result) => {
            if !result.is_empty() {
                msg_warn!(
                    "{} 检查器在文件 {} 中检测到 {} 个问题，下面是详细信息",
                    checker.manifest().name,
                    format!(
                        "{}",
                        path.strip_prefix(&gctx().config.as_ref().unwrap().0.path)
                            .unwrap()
                            .display()
                    )
                    .cyan(),
                    result.len()
                );
                for message in result {
                    if let Some(col) = message.col
                        && let Some(line) = message.line
                    {
                        msg_warn!(
                            "在 {}:{},{} 等级 {}, 消息: {}",
                            format!(
                                "{}",
                                path.strip_prefix(&gctx().config.as_ref().unwrap().0.path)
                                    .unwrap()
                                    .display()
                            )
                            .cyan(),
                            col.to_string().magenta(),
                            line.to_string().magenta(),
                            match message.importance {
                                CheckImportance::Warn => "警告".yellow(),
                                CheckImportance::Error => "错误".red(),
                            },
                            message.info
                        )
                    } else {
                        msg_warn!(
                            "在 {} 等级 {}, 消息: {}",
                            format!(
                                "{}",
                                path.strip_prefix(&gctx().config.as_ref().unwrap().0.path)
                                    .unwrap()
                                    .display()
                            )
                            .cyan(),
                            match message.importance {
                                CheckImportance::Warn => "警告".yellow(),
                                CheckImportance::Error => "错误".red(),
                            },
                            message.info
                        )
                    }
                }
            }
        }
    }
}

pub fn check(problem_config: &ProblemConfig) -> Result<()> {
    let markdown_path = problem_config.path.join("statement.md");

    let markdown_text = fs::read_to_string(&markdown_path)?;

    let state = MarkdownParserState::new();
    let ast = match parse_markdown(state, &markdown_text) {
        Ok(val) => val,
        Err(_) => bail!("解析题面文件失败"),
    };

    let checkers = get_checkers();

    for checker in &checkers {
        if checker.manifest().markdown_checker {
            debug!("正在应用文本检查器 {}", checker.manifest().name);
            let messages = checker.check_markdown(&markdown_text, problem_config)?;
            print_messages(messages, &markdown_path, checker.as_ref());
        }
    }

    for checker in &checkers {
        if checker.manifest().ast_checker {
            debug!("正在应用检查器 {}", checker.manifest().name);
            let messages = checker.check_ast(&ast, problem_config)?;
            print_messages(messages, &markdown_path, checker.as_ref());
        }
    }

    Ok(())
}

pub fn check_day(day_config: &ContestDayConfig) -> Result<()> {
    for (_, problem_config) in &day_config.subconfig {
        check(problem_config)?;
    }
    Ok(())
}

fn explain(id: String) -> Result<()> {
    let checkers = get_checkers();

    for checker in checkers {
        if checker.manifest().name == id {
            println!("规则 {}: {}", id, checker.manifest().description);
            return Ok(());
        }
    }

    bail!("找不到规则 {}", id);
}

pub fn main(args: CheckArgs) -> Result<()> {
    if let Some(rule) = args.explain {
        explain(rule)?;
        return Ok(());
    }

    let config = gctx().config.as_ref().context("没有可用的工程")?;

    match &config.1 {
        CurrentLocation::None => bail!("没有可用的工程"),
        CurrentLocation::Root => {
            for (_, day_config) in &config.0.subconfig {
                check_day(day_config)?;
            }
        }
        CurrentLocation::Day(day) => {
            check_day(config.0.subconfig.get(day).unwrap())?;
        }
        CurrentLocation::Problem(day, problem) => {
            check(
                config
                    .0
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
