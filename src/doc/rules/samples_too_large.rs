use super::FormatRule;
use crate::{
    doc::rules::{
        CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule, FormatManifest,
    },
    prelude::*,
};
use lazy_static::lazy_static;
use markdown_ppp::ast::*;
use regex::Regex;

lazy_static! {
    static ref SAMPLE_TEXT_PATTERN: Regex =
        Regex::new(r"\{\{\s*sample\.text\((\d+)\)\s*\}\}").unwrap();
}

/// 检查文件是否超过限制
fn check_file_limits(path: &std::path::Path) -> Result<(bool, Option<String>)> {
    if !path.exists() {
        return Ok((false, None));
    }

    let content = std::fs::read_to_string(path)?;
    let line_count = content.lines().count();
    let max_line_length = content
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let size_bytes = content.len();

    let mut violations = Vec::new();
    if line_count > 80 {
        violations.push(format!("{}行", line_count));
    }
    if max_line_length > 40 {
        violations.push(format!("{}列", max_line_length));
    }
    if size_bytes > 10 * 1024 {
        violations.push(format!("{:.2}KB", size_bytes as f64 / 1024.0));
    }

    if violations.is_empty() {
        Ok((false, None))
    } else {
        Ok((true, Some(violations.join("、"))))
    }
}

pub struct SamplesTooLarge;

impl FormatRule for SamplesTooLarge {
    fn manifest(&self) -> FormatManifest {
        FormatManifest {
            name: "samples-too-large".to_string(),
            description: "将超过限制的 sample.text 替换为 sample.file".to_string(),
            markdown_formatter: true,
            ast_formatter: false,
        }
    }

    fn apply_markdown(
        &self,
        markdown_text: String,
        problem_config: ProblemConfig,
    ) -> Result<(String, ProblemConfig)> {
        let mut result = markdown_text.clone();

        // 查找文档中所有的 sample.text
        let matches: Vec<_> = SAMPLE_TEXT_PATTERN
            .captures_iter(&markdown_text)
            .filter_map(|caps| {
                let full_match = caps.get(0).unwrap();
                let id = caps.get(1).unwrap().as_str().parse::<u32>().ok()?;

                // 查找对应的样本配置
                let sample = problem_config.samples.iter().find(|s| s.id == id)?;

                let mut should_replace = false;

                // 检查输入文件
                if let Some(input_path) = sample.input.get() {
                    let path = problem_config.path.join("sample").join(input_path);
                    if let Ok((exceed, _)) = check_file_limits(&path) {
                        if exceed {
                            should_replace = true;
                        }
                    }
                }

                // 检查输出文件
                if let Some(output_path) = sample.output.get() {
                    let path = problem_config.path.join("sample").join(output_path);
                    if let Ok((exceed, _)) = check_file_limits(&path) {
                        if exceed {
                            should_replace = true;
                        }
                    }
                }

                if should_replace {
                    Some((full_match.start(), full_match.end(), id))
                } else {
                    None
                }
            })
            .collect();

        // 从后往前替换
        for (start, end, id) in matches.into_iter().rev() {
            let replacement = format!("{{{{ sample.file({}) }}}}", id);
            result.replace_range(start..end, &replacement);
        }

        Ok((result, problem_config))
    }

    fn apply_ast(&self, _: Document, _: ProblemConfig) -> Result<(Document, ProblemConfig)> {
        unreachable!()
    }
}

impl CheckRule for SamplesTooLarge {
    fn manifest(&self) -> CheckManifest {
        CheckManifest {
            name: "samples-too-large".to_string(),
            description: "检查 sample.text 是否超过 80 行/40 列/10 KB 限制".to_string(),
            markdown_checker: true,
            ast_checker: false,
        }
    }

    fn check_markdown(
        &self,
        markdown_text: &String,
        problem_config: &ProblemConfig,
    ) -> Result<CheckResult> {
        let mut messages: Vec<CheckInfo> = vec![];

        // 查找文档中所有的 sample.text
        for caps in SAMPLE_TEXT_PATTERN.captures_iter(markdown_text) {
            let full_match = caps.get(0).unwrap();
            let id = caps.get(1).unwrap().as_str().parse::<u32>().unwrap_or(0);
            let col_num = full_match.start();

            // 查找对应的样本配置
            if let Some(sample) = problem_config.samples.iter().find(|s| s.id == id) {
                let mut violations = Vec::new();

                // 检查输入文件
                if let Some(input_path) = sample.input.get() {
                    let path = problem_config.path.join("sample").join(input_path);
                    if let Ok((exceed, violation)) = check_file_limits(&path) {
                        if exceed {
                            violations.push(format!(
                                "输入文件 {} ({})",
                                input_path,
                                violation.unwrap()
                            ));
                        }
                    }
                }

                // 检查输出文件
                if let Some(output_path) = sample.output.get() {
                    let path = problem_config.path.join("sample").join(output_path);
                    if let Ok((exceed, violation)) = check_file_limits(&path) {
                        if exceed {
                            violations.push(format!(
                                "输出文件 {} ({})",
                                output_path,
                                violation.unwrap()
                            ));
                        }
                    }
                }

                if !violations.is_empty() {
                    messages.push(CheckInfo {
                        line: None, // 可以传 None，或者从上下文中获取行号
                        col: Some(col_num + 1),
                        info: format!(
                            "sample.text({}) 的 {} 超过限制，建议替换为 sample.file({})",
                            id,
                            violations.join("和"),
                            id
                        ),
                        importance: CheckImportance::Warn,
                    });
                }
            }
        }

        Ok(CheckResult::Tagged(messages))
    }

    fn check_ast(&self, _: &Document, _: &ProblemConfig) -> Result<CheckResult> {
        unreachable!()
    }
}
