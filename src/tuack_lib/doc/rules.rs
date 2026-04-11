use crate::prelude::*;
use markdown_ppp::ast::Document;

// Format

pub struct FormatManifest {
    pub name: String,
    pub description: String,
    pub markdown_formatter: bool,
    pub ast_formatter: bool,
}

pub trait FormatRule {
    fn apply_markdown(
        &self,
        doc: String,
        problem_config: ProblemConfig,
    ) -> Result<(String, ProblemConfig)>;
    fn apply_ast(
        &self,
        doc: Document,
        problem_config: ProblemConfig,
    ) -> Result<(Document, ProblemConfig)>;
    fn manifest(&self) -> FormatManifest;
}

// Check
#[derive(PartialEq)]
pub enum CheckImportance {
    Warn,
    Error,
}

pub struct CheckInfo {
    pub line: Option<usize>,
    pub col: Option<usize>,
    pub info: String,
    pub importance: CheckImportance,
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
    fn check_ast(&self, doc: &Document, problem_config: &ProblemConfig) -> Result<CheckResult>;
    fn manifest(&self) -> CheckManifest;
}
