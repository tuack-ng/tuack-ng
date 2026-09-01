use crate::prelude::*;
use tuack_ng_parser::ast::Document;

pub mod autocorrect;
pub mod html;
pub mod invisible;
pub mod latex;
pub mod samples_not_found;
pub mod samples_should_be_external;
pub mod samples_too_large;

// Format

pub struct FormatManifest {
    pub name: String,
    pub description: String,
    pub markdown_formatter: bool,
    pub ast_formatter: bool,
}

/// 规则产出的待落盘文件（`path` 相对题目根，如 `sample/1.in`）。
/// 产生的文件随结果返回，由调用方统一处理。
pub struct RuleFile {
    pub path: PathBuf,
    pub content: Vec<u8>,
}

pub trait FormatRule {
    fn apply_markdown(
        &self,
        doc: String,
        problem_config: ProblemConfig,
    ) -> Result<(String, ProblemConfig, Vec<RuleFile>)>;
    fn apply_ast(
        &self,
        doc: Document,
        problem_config: ProblemConfig,
    ) -> Result<(Document, ProblemConfig, Vec<RuleFile>)>;
    fn manifest(&self) -> FormatManifest;
}

// Check
#[derive(Clone, Copy, PartialEq)]
pub enum CheckImportance {
    Warn,
    Error,
}

pub struct CheckInfo {
    pub span: Option<tuack_ng_parser::span::Span>,
    pub secondary_span: Option<tuack_ng_parser::span::Span>,
    pub info: String,
    pub note: Option<String>,
    pub importance: CheckImportance,
}

impl CheckInfo {
    pub fn new(
        span: Option<tuack_ng_parser::span::Span>,
        info: String,
        importance: CheckImportance,
    ) -> Self {
        Self {
            span,
            secondary_span: None,
            info,
            note: None,
            importance,
        }
    }
}

pub enum CheckResult {
    #[allow(unused)]
    Untagged(usize),
    Tagged(Vec<CheckInfo>),
}

pub struct CheckManifest {
    pub name: String,
    pub description: String,
    pub markdown_checker: bool,
    pub ast_checker: bool,
}

pub trait CheckRule {
    fn check_markdown(&self, doc: &str, problem_config: &ProblemConfig) -> Result<CheckResult>;
    fn check_ast(
        &self,
        doc: &Document,
        source: &str,
        problem_config: &ProblemConfig,
    ) -> Result<CheckResult>;
    fn manifest(&self) -> CheckManifest;
}
