use super::FormatRule;
use crate::{
    doc::rules::{
        CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule, FormatManifest,
    },
    prelude::*,
};
use markdown_ppp::ast::*;

// 不可见字符列表
const INVISIBLE_CHARS: &[char] = &[
    '\u{200B}', // 零宽空格 (Zero-width space)
    '\u{200C}', // 零宽非连接符 (Zero-width non-joiner)
    '\u{200D}', // 零宽连接符 (Zero-width joiner)
    '\u{FEFF}', // 零宽非断空格/字节顺序标记 (Zero-width no-break space / BOM)
    '\u{00AD}', // 软连字符 (Soft hyphen)
    '\u{2060}', // 词连接符 (Word joiner)
    '\u{180E}', // 蒙古语元音分隔符 (Mongolian vowel separator)
    '\u{034F}', // 组合用希腊语符号 (Combining grapheme joiner)
    '\u{200E}', // 左至右标记 (Left-to-right mark)
    '\u{200F}', // 右至左标记 (Right-to-left mark)
    '\u{202A}', // 左至右嵌入 (Left-to-right embedding)
    '\u{202B}', // 右至左嵌入 (Right-to-left embedding)
    '\u{202C}', // 弹出方向格式化 (Pop directional formatting)
    '\u{202D}', // 左至右覆盖 (Left-to-right override)
    '\u{202E}', // 右至左覆盖 (Right-to-left override)
];

/// 移除文本中的不可见字符
fn remove_invisible_chars(text: &str) -> String {
    text.chars()
        .filter(|c| !INVISIBLE_CHARS.contains(c))
        .collect()
}

/// 获取不可见字符的名称
fn get_char_name(c: char) -> &'static str {
    match c {
        '\u{200B}' => "零宽空格",
        '\u{200C}' => "零宽非连接符",
        '\u{200D}' => "零宽连接符",
        '\u{FEFF}' => "零宽非断空格/BOM",
        '\u{00AD}' => "软连字符",
        '\u{2060}' => "词连接符",
        '\u{180E}' => "蒙古语元音分隔符",
        '\u{034F}' => "组合用希腊语符号",
        '\u{200E}' => "左至右标记",
        '\u{200F}' => "右至左标记",
        '\u{202A}' => "左至右嵌入",
        '\u{202B}' => "右至左嵌入",
        '\u{202C}' => "弹出方向格式化",
        '\u{202D}' => "左至右覆盖",
        '\u{202E}' => "右至左覆盖",
        _ => "未知不可见字符",
    }
}

pub struct Invisible;

impl FormatRule for Invisible {
    fn manifest(&self) -> FormatManifest {
        FormatManifest {
            name: "invisible".to_string(),
            description: "移除文本中的不可见字符".to_string(),
            markdown_formatter: true,
            ast_formatter: false,
        }
    }

    fn apply_markdown(
        &self,
        markdown_text: String,
        problem_config: ProblemConfig,
    ) -> Result<(String, ProblemConfig)> {
        let cleaned_text = remove_invisible_chars(&markdown_text);
        Ok((cleaned_text, problem_config))
    }

    fn apply_ast(&self, _: Document, _: ProblemConfig) -> Result<(Document, ProblemConfig)> {
        unreachable!()
    }
}

impl CheckRule for Invisible {
    fn manifest(&self) -> CheckManifest {
        CheckManifest {
            name: "invisible".to_string(),
            description: "检查文本中的不可见字符".to_string(),
            markdown_checker: true,
            ast_checker: false,
        }
    }

    fn check_markdown(&self, markdown_text: &str, _: &ProblemConfig) -> Result<CheckResult> {
        let mut messages: Vec<CheckInfo> = vec![];

        for (line_num, line) in markdown_text.lines().enumerate() {
            let line_number = line_num + 1;

            for (col_num, c) in line.chars().enumerate() {
                if INVISIBLE_CHARS.contains(&c) {
                    messages.push(CheckInfo {
                        line: Some(line_number),
                        col: Some(col_num + 1),
                        info: format!("发现不可见字符: {} (U+{:04X})", get_char_name(c), c as u32),
                        importance: CheckImportance::Error,
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
