use crate::{
    doc::rules::{CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule},
    prelude::*,
};
use lazy_static::lazy_static;
use regex::Regex;
use tuack_ng_parser::ast::Document;
use tuack_ng_parser::ast::block::BlockKind;
use tuack_ng_parser::ast::inline::InlineKind;
use tuack_ng_parser::span::Span;
use tuack_ng_parser::visitor::{VisitWith, Visitor};

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

    // 除号 ÷
    static ref DIVISION_SIGN: Regex = Regex::new(r"÷").unwrap();

    // \frac{...}{...} 及其变体（dfrac/tfrac/cfrac），也支持 \frac ab 的简写形式
    static ref FRACTION_CMD: Regex =
        Regex::new(r"\\[a-z]*frac(\s*\{[^{}]*\}|\s*\S)(\s*\{[^{}]*\}|\s*\S)").unwrap();

    // mod 运算符
    static ref MOD_OPERATOR: Regex = Regex::new(r"\bmod\b").unwrap();

    // 汉字和中文标点
    static ref CHINESE_CHARS: Regex = Regex::new(r"[\u4e00-\u9fff\u3000-\u303f\uff00-\uffef]").unwrap();

    // 大数字 (6 位或以上)
    static ref LARGE_NUMBER: Regex = Regex::new(r"\b\d{6,}\b").unwrap();

    // 带逗号的数字
    static ref COMMA_NUMBER: Regex = Regex::new(r"\d{3,},\d{3,}").unwrap();
}

/// 一条 LaTeX 格式违规
struct LatexProblem {
    info: String,
    span: Option<Span>,
    importance: CheckImportance,
}

/// LaTeX 检查器
/// 行内逐条定位到匹配子串，块级汇总为一条并附 note。
struct LatexVisitor {
    messages: Vec<CheckInfo>,
}

impl LatexVisitor {
    /// 计算 content 内 `[start, end)`（字节偏移）在源码中的区间。
    /// `base` 为 content 首字节在源码中的偏移；为 None 时无精确映射（返回 None）。
    fn cap_span(base: Option<usize>, start: usize, end: usize) -> Option<Span> {
        base.map(|b| Span::new(b + start, b + end))
    }

    /// 数字 `[start,end)` 是否位于 `tools.hn(` / `tools.comma(` 调用内部。
    /// 被模板数字工具包装的数字不应触发「数字太长」告警。
    fn in_tools_number(text: &str, start: usize, end: usize) -> bool {
        let before = &text[..start];
        let after = &text[end..];
        for prefix in ["tools.hn(", "tools.comma("] {
            let Some(open) = before.rfind(prefix) else {
                continue;
            };
            // 同一括号对：open 至数字前不应出现 `)`（数字必须仍在调用括号内）。
            if before[open..].contains(')') {
                continue;
            }
            if after.find(')').is_some() {
                return true;
            }
        }
        false
    }

    /// 检测公式文本，返回违规列表（不做 push，供行内逐条 / 块级汇总复用）。
    fn check_latex(&self, latex: &str, base: Option<usize>) -> Vec<LatexProblem> {
        let mut problems: Vec<LatexProblem> = Vec::new();

        // 数学函数名（应该用 \func 而不是 func）
        for cap in MATH_FUNCTIONS.find_iter(latex) {
            let func = cap.as_str();
            let start = cap.start();
            if start == 0 || latex.as_bytes().get(start - 1).is_none_or(|&b| b != b'\\') {
                problems.push(LatexProblem {
                    info: format!("`{}` 应该写成 `\\{}`", func, func),
                    span: Self::cap_span(base, cap.start(), cap.end()),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 小于等于符号
        for cap in LE_OPERATOR.find_iter(latex) {
            problems.push(LatexProblem {
                info: format!("`{}` 应该写成 `\\le`", cap.as_str()),
                span: Self::cap_span(base, cap.start(), cap.end()),
                importance: CheckImportance::Warn,
            });
        }

        // 大于等于符号
        for cap in GE_OPERATOR.find_iter(latex) {
            problems.push(LatexProblem {
                info: format!("`{}` 应该写成 `\\ge`", cap.as_str()),
                span: Self::cap_span(base, cap.start(), cap.end()),
                importance: CheckImportance::Warn,
            });
        }

        // 省略号
        for cap in ELLIPSIS.find_iter(latex) {
            problems.push(LatexProblem {
                info: format!(
                    "`{}` 应该写成 `\\dots`（逗号分隔）或 `\\cdots`（运算符分隔）",
                    cap.as_str()
                ),
                span: Self::cap_span(base, cap.start(), cap.end()),
                importance: CheckImportance::Warn,
            });
        }

        // mod 运算符
        for cap in MOD_OPERATOR.find_iter(latex) {
            let start = cap.start();
            if start == 0 || latex.as_bytes().get(start - 1).is_none_or(|&b| b != b'\\') {
                problems.push(LatexProblem {
                    info: "`mod` 应该写成 `\\bmod` 或 `\\pmod{}`".to_string(),
                    span: Self::cap_span(base, cap.start(), cap.end()),
                    importance: CheckImportance::Warn,
                });
            }
        }

        // 乘号（星号），排除指数中的 * （如 10^{*}）
        let cleaned = latex.replace("^{*}", "").replace("^*", "");
        if MULTIPLY_STAR.is_match(&cleaned) {
            problems.push(LatexProblem {
                info: "一般不用星号 `*` 做乘号，应该用 `\\times`（叉乘）、`\\cdot`（点乘）或省略"
                    .to_string(),
                span: base.map(|b| Span::new(b, b + latex.len())),
                importance: CheckImportance::Warn,
            });
        }

        // 除号 ÷
        for cap in DIVISION_SIGN.find_iter(latex) {
            problems.push(LatexProblem {
                info: format!("`{}` 应该写成 `\\div`", cap.as_str()),
                span: Self::cap_span(base, cap.start(), cap.end()),
                importance: CheckImportance::Warn,
            });
        }

        // 除号（斜杠），先剔除 \frac 等命令本身
        let without_fraction = FRACTION_CMD.replace_all(latex, "");
        if DIVIDE_SLASH.is_match(&without_fraction) {
            problems.push(LatexProblem {
                info: "一般不用斜杠 `/` 做除号，应该用 `\\frac{}{}` 或 `\\div`".to_string(),
                span: base.map(|b| Span::new(b, b + latex.len())),
                importance: CheckImportance::Warn,
            });
        }

        // 汉字和中文标点
        for cap in CHINESE_CHARS.find_iter(latex) {
            problems.push(LatexProblem {
                info: format!("不能包含汉字或中文标点 `{}`", cap.as_str()),
                span: Self::cap_span(base, cap.start(), cap.end()),
                importance: CheckImportance::Error,
            });
        }

        // 大数字
        for cap in LARGE_NUMBER.find_iter(latex) {
            if Self::in_tools_number(latex, cap.start(), cap.end()) {
                continue;
            }
            problems.push(LatexProblem {
                info: format!(
                    "数字 `{}` 太长，建议用科学计数法（如 `10^6`）、定义为变量，或使用 tools.hn/comma",
                    cap.as_str()
                ),
                span: Self::cap_span(base, cap.start(), cap.end()),
                importance: CheckImportance::Warn,
            });
        }

        // 带逗号的大数字
        for cap in COMMA_NUMBER.find_iter(latex) {
            if Self::in_tools_number(latex, cap.start(), cap.end()) {
                continue;
            }
            problems.push(LatexProblem {
                info: format!(
                    "数字 `{}` 太长，建议用科学计数法、定义为变量，或使用 tools.hn/comma",
                    cap.as_str()
                ),
                span: Self::cap_span(base, cap.start(), cap.end()),
                importance: CheckImportance::Warn,
            });
        }

        // 前后空格
        if latex.starts_with(' ') || latex.ends_with(' ') {
            problems.push(LatexProblem {
                info: "前后不应该有空格".to_string(),
                span: base.map(|b| Span::new(b, b + latex.len())),
                importance: CheckImportance::Error,
            });
        }

        problems
    }
}

impl Visitor for LatexVisitor {
    fn visit_inline(&mut self, inline: &tuack_ng_parser::Inline) {
        if let InlineKind::Latex(content) = &inline.value {
            // 行内 content 首字节为 `$` 之后
            let base = inline.span.map(|s| s.start + 1);
            for problem in self.check_latex(content, base) {
                self.messages.push(CheckInfo {
                    span: problem.span.or(inline.span),
                    secondary_span: None,
                    info: format!("在公式 {} 中：{}", content, problem.info),
                    note: None,
                    importance: problem.importance,
                });
            }
        }
        self.walk_inline(inline);
    }
    fn visit_block(&mut self, block: &tuack_ng_parser::Block) {
        if let BlockKind::LatexBlock(content) = &block.value {
            let problems = self.check_latex(content, None);
            if !problems.is_empty() {
                let importance = if problems
                    .iter()
                    .any(|p| p.importance == CheckImportance::Error)
                {
                    CheckImportance::Error
                } else {
                    CheckImportance::Warn
                };
                let note = Some(
                    problems
                        .iter()
                        .map(|p| format!("- {}", p.info))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
                self.messages.push(CheckInfo {
                    span: block.span,
                    secondary_span: None,
                    info: format!("块级 LaTeX 公式中发现 {} 处格式问题", problems.len()),
                    note,
                    importance,
                });
            }
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

    fn check_markdown(&self, _: &str, _: &ProblemConfig) -> Result<CheckResult> {
        unreachable!()
    }

    fn check_ast(
        &self,
        doc: &Document,
        _source: &str,
        _problem_config: &ProblemConfig,
    ) -> Result<CheckResult> {
        let mut visitor = LatexVisitor {
            messages: Vec::new(),
        };
        doc.visit_with(&mut visitor);
        Ok(CheckResult::Tagged(visitor.messages))
    }
}
