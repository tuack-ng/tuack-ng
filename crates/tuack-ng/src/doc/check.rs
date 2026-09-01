use crate::prelude::*;
use clap::Args;
use codespan_reporting::diagnostic::{Diagnostic, Label, LabelStyle, Severity};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::{Chars, Config, emit_to_write_style};
use termcolor::Buffer;
use tuack_ng_parser::parse;
use tuack_utils::doc::rules::*;
use tuack_utils::doc::rules::{
    autocorrect, html, invisible, latex, samples_not_found, samples_should_be_external,
    samples_too_large,
};

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

fn severity(importance: CheckImportance) -> Severity {
    match importance {
        CheckImportance::Error => Severity::Error,
        CheckImportance::Warn => Severity::Warning,
    }
}

/// 计算整行字节区间（不含换行符），用于 span 缺失/零长度时的兜底定位。
fn whole_line_range(markdown: &str, offset: Option<usize>) -> std::ops::Range<usize> {
    let target = offset.unwrap_or(0).min(markdown.len());
    let line_start = markdown[..target].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = markdown[target..]
        .find('\n')
        .map(|i| target + i)
        .unwrap_or(markdown.len());
    line_start..line_end
}

fn render_message(markdown: &str, path: &Path, rule_id: &str, message: &CheckInfo) {
    let display = path
        .strip_prefix(&gctx().config.as_ref().unwrap().config.path)
        .unwrap_or(path)
        .display()
        .to_string();

    let range = match message.span {
        Some(span) if span.stop > span.start => span.start..span.stop,
        span => whole_line_range(markdown, span.map(|s| s.start)),
    };

    let mut labels = vec![Label::new(LabelStyle::Primary, (), range)];
    if let Some(secondary) = message.secondary_span {
        let secondary_range = if secondary.stop > secondary.start {
            secondary.start..secondary.stop
        } else {
            whole_line_range(markdown, Some(secondary.start))
        };
        labels.push(
            Label::new(LabelStyle::Secondary, (), secondary_range).with_message("样例输出块在此"),
        );
    }

    let mut diagnostic = Diagnostic::new(severity(message.importance))
        .with_code(rule_id)
        .with_message(&message.info)
        .with_labels(labels);
    if let Some(note) = &message.note {
        diagnostic = diagnostic.with_note(note.clone());
    }

    let files = SimpleFile::new(display, markdown);
    let mut buffer = Buffer::ansi();
    let config = Config {
        chars: Chars::box_drawing(),
        ..Config::default()
    };
    if emit_to_write_style(&mut buffer, &config, &files, &diagnostic).is_ok()
        && let Ok(s) = String::from_utf8(buffer.into_inner())
    {
        crate::_internal_print!(eprintln, "{}", s);
    }
}

fn print_messages(messages: CheckResult, path: &Path, checker: &dyn CheckRule, markdown: &str) {
    match messages {
        CheckResult::Untagged(num) => {
            if num > 0 {
                msg_warn!(
                    "{} 检查器在文件 {} 中检测到 {} 个问题。使用 `doc format` 来修复",
                    checker.manifest().name.green(),
                    format!(
                        "{}",
                        path.strip_prefix(&gctx().config.as_ref().unwrap().config.path)
                            .unwrap()
                            .display()
                    )
                    .cyan(),
                    num
                );
            }
        }
        CheckResult::Tagged(result) => {
            for message in result {
                render_message(markdown, path, &checker.manifest().name, &message);
            }
        }
    }
}

pub fn check(problem_config: &ProblemConfig) -> Result<()> {
    let markdown_path = problem_config.path.join("statement.md");

    let markdown_text = fs::read_to_string(&markdown_path)?;

    let ast = parse(&markdown_text);

    let checkers = get_checkers();

    for checker in &checkers {
        // 每个规则先应用文本检查，再应用 AST 检查
        if checker.manifest().markdown_checker {
            debug!("正在应用文本检查器 {}", checker.manifest().name);
            let messages = checker.check_markdown(&markdown_text, problem_config)?;
            print_messages(messages, &markdown_path, checker.as_ref(), &markdown_text);
        }

        if checker.manifest().ast_checker {
            debug!("正在应用检查器 {}", checker.manifest().name);
            let messages = checker.check_ast(&ast, &markdown_text, problem_config)?;
            print_messages(messages, &markdown_path, checker.as_ref(), &markdown_text);
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

    match &config.location {
        CurrentLocation::None => bail!("没有可用的工程"),
        CurrentLocation::Root => {
            for (_, day_config) in &config.config.subconfig {
                check_day(day_config)?;
            }
        }
        CurrentLocation::Day(day) => {
            check_day(config.config.subconfig.get(day).unwrap())?;
        }
        CurrentLocation::Problem(day, problem) => {
            check(
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
