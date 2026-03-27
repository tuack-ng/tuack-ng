use crate::{doc::rules::*, prelude::*};
use markdown_ppp::parser::*;

fn get_checkers() -> Vec<Box<dyn CheckRule>> {
    let mut checkers: Vec<Box<dyn CheckRule>> = vec![];
    checkers.push(Box::new(invisible::Invisible));
    checkers.push(Box::new(
        samples_should_be_external::SamplesShouldBeExternal,
    ));
    checkers.push(Box::new(autocorrect::Autocorrect));
    checkers.push(Box::new(latex::Latex));

    checkers
}

fn print_messages(messages: CheckResult, path: &PathBuf, checker: &Box<dyn CheckRule>) {
    match messages {
        CheckResult::Untagged(num) => {
            if num > 0 {
                warn!(
                    "{} 检查器在文件 {} 中检测到 {} 个问题。使用 `doc format` 来修复",
                    checker.manifest().name,
                    path.display(),
                    num
                );
            }
        }
        CheckResult::Tagged(result) => {
            if result.len() > 0 {
                warn!(
                    "{} 检查器在文件 {} 中检测到 {} 个问题，下面是详细信息",
                    checker.manifest().name,
                    path.display(),
                    result.len()
                );
                for message in result {
                    if message.col.is_some() && message.line.is_some() {
                        warn!(
                            "在 {}:{},{} 等级 {}, 消息: {}",
                            path.display(),
                            message.line.unwrap(),
                            message.col.unwrap(),
                            match message.importance {
                                CheckImportance::Warn => "警告",
                                CheckImportance::Error => "错误",
                            },
                            message.info
                        )
                    } else {
                        warn!(
                            "在 {} 等级 {}, 消息: {}",
                            path.display(),
                            match message.importance {
                                CheckImportance::Warn => "警告",
                                CheckImportance::Error => "错误",
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
            print_messages(messages, &markdown_path, checker);
        }
    }

    for checker in &checkers {
        if checker.manifest().ast_checker {
            debug!("正在应用检查器 {}", checker.manifest().name);
            let messages = checker.check_ast(&ast, problem_config)?;
            print_messages(messages, &markdown_path, checker);
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

pub fn main() -> Result<()> {
    let config = get_context().config.as_ref().context("没有可用的工程")?;

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
                &config
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
