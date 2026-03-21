use crate::{
    doc::rules::{CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule},
    prelude::*,
};
use lazy_static::lazy_static;
use markdown_ppp::{
    ast::*,
    ast_transform::{VisitWith, Visitor},
};
use regex::Regex;

lazy_static! {
    // 数学函数名
    static ref MATH_FUNCTIONS: Regex = Regex::new(
        r"\b(sin|cos|tan|cot|sec|csc|log|ln|lg|min|max|gcd|lcm|exp|lim|inf|sup)\b"
    ).unwrap();

    // 关系运算符
    static ref LE_OPERATOR: Regex = Regex::new(r"<=|≤").unwrap();
    static ref GE_OPERATOR: Regex = Regex::new(r">=|≥").unwrap();

    // 省略号
    static ref ELLIPSIS: Regex = Regex::new(r"\.\.\.|…").unwrap();

    // 乘号
    static ref MULTIPLY_STAR: Regex = Regex::new(r"\*").unwrap();

    // 除号
    static ref DIVIDE_SLASH: Regex = Regex::new(r"/").unwrap();

    // mod 运算符
    static ref MOD_OPERATOR: Regex = Regex::new(r"\bmod\b").unwrap();

    // 汉字和中文标点
    static ref CHINESE_CHARS: Regex = Regex::new(r"[\u4e00-\u9fff\u3000-\u303f\uff00-\uffef]").unwrap();

    // 大数字 (6位或以上)
    static ref LARGE_NUMBER: Regex = Regex::new(r"\b\d{6,}\b").unwrap();

    // 带逗号的数字
    static ref COMMA_NUMBER: Regex = Regex::new(r"\d{3,},\d{3,}").unwrap();
}

struct LatexVisitor {
    messages: Vec<CheckInfo>,
}

impl LatexVisitor {
    fn check_latex(&mut self, latex: &String) {
        // 检查数学函数名（应该用 \sin 而不是 sin）
        for cap in MATH_FUNCTIONS.find_iter(latex) {
            let func = cap.as_str();
            // 检查是否已经有反斜杠
            let start = cap.start();
            if start == 0
                || !latex
                    .as_bytes()
                    .get(start - 1)
                    .map_or(false, |&b| b == b'\\')
            {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!("在公式 {} 中: `{}` 应该写成 `\\{}`", latex, func, func),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 检查小于等于符号
        if LE_OPERATOR.is_match(latex) {
            for cap in LE_OPERATOR.find_iter(latex) {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!("在公式 {} 中: `{}` 应该写成 `\\le`", latex, cap.as_str()),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 检查大于等于符号
        if GE_OPERATOR.is_match(latex) {
            for cap in GE_OPERATOR.find_iter(latex) {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!("在公式 {} 中: `{}` 应该写成 `\\ge`", latex, cap.as_str()),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 检查省略号
        if ELLIPSIS.is_match(latex) {
            for cap in ELLIPSIS.find_iter(latex) {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!(
                        "在公式 {} 中: `{}` 应该写成 `\\dots`（逗号分隔）或 `\\cdots`（运算符分隔）",
                        latex, cap.as_str()
                    ),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 检查 mod 运算符
        if MOD_OPERATOR.is_match(latex) {
            for cap in MOD_OPERATOR.find_iter(latex) {
                let start = cap.start();
                // 检查是否已经有反斜杠（\bmod 或 \pmod）
                if start == 0
                    || !latex
                        .as_bytes()
                        .get(start - 1)
                        .map_or(false, |&b| b == b'\\')
                {
                    self.messages.push(CheckInfo {
                        line: None,
                        col: None,
                        info: format!(
                            "在公式 {} 中: `mod` 应该写成 `\\bmod` 或 `\\pmod{{}}`",
                            latex
                        ),
                        importance: CheckImportance::Warn,
                    });
                }
            }
        }

        // 检查乘号（星号）
        if MULTIPLY_STAR.is_match(latex) {
            // 排除指数中的 * （如 10^{*}）
            let cleaned = latex.replace("^{*}", "").replace("^*", "");
            if MULTIPLY_STAR.is_match(&cleaned) {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!(
                        "在公式 {} 中: 一般不用星号 `*` 做乘号，应该用 `\\times`（叉乘）、`\\cdot`（点乘）或省略",
                        latex
                    ),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 检查除号（斜杠）
        if DIVIDE_SLASH.is_match(latex) {
            // 排除已经在 \frac 等命令中的
            if !latex.contains("\\frac") {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!(
                        "在公式 {} 中: 一般不用斜杠 `/` 做除号，应该用 `\\frac{{}}{{}}` 或 `\\div`",
                        latex
                    ),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 检查汉字和中文标点
        if CHINESE_CHARS.is_match(latex) {
            for cap in CHINESE_CHARS.find_iter(latex) {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!(
                        "在公式 {} 中: 不能包含汉字或中文标点 `{}`",
                        latex,
                        cap.as_str()
                    ),
                    importance: CheckImportance::Error,
                });
            }
        }

        // 检查大数字
        if LARGE_NUMBER.is_match(latex) {
            for cap in LARGE_NUMBER.find_iter(latex) {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!(
                        "在公式 {} 中: 数字 `{}` 太长，建议用科学计数法（如 `10^6`）或定义为变量",
                        latex,
                        cap.as_str()
                    ),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 检查带逗号的大数字
        if COMMA_NUMBER.is_match(latex) {
            for cap in COMMA_NUMBER.find_iter(latex) {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!(
                        "在公式 {} 中: 数字 `{}` 太长，建议用科学计数法或定义为变量",
                        latex,
                        cap.as_str()
                    ),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 检查前后空格
        if latex.starts_with(' ') || latex.ends_with(' ') {
            self.messages.push(CheckInfo {
                line: None,
                col: None,
                info: format!("行内公式 {} 的前后不应该有空格（紧贴  符号）", latex),
                importance: CheckImportance::Error,
            });
        }
    }
}

impl Visitor for LatexVisitor {
    fn visit_inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Latex(content) => self.check_latex(content),
            _ => {}
        }
        self.walk_inline(inline);
    }
    fn visit_block(&mut self, block: &Block) {
        match block {
            Block::LatexBlock(content) => self.check_latex(content),
            _ => {}
        }
        self.walk_block(block);
    }
}

pub struct Latex;

impl CheckRule for Latex {
    fn manifest(&self) -> CheckManifest {
        CheckManifest {
            name: "latex".to_string(),
            description: "检测 LaTeX 公式格式问题（函数名、运算符、汉字、数字等）".to_string(),
            markdown_checker: false,
            ast_checker: true,
        }
    }

    fn check_markdown(&self, _: &String, _: &ProblemConfig) -> Result<CheckResult> {
        unreachable!()
    }

    fn check_ast(&self, doc: &Document, _problem_config: &ProblemConfig) -> Result<CheckResult> {
        let mut visitor = LatexVisitor {
            messages: Vec::new(),
        };
        doc.visit_with(&mut visitor);
        Ok(CheckResult::Tagged(visitor.messages))
    }
}
