use crate::prelude::*;
use markdown_ppp::ast::Document;

pub mod samples_should_be_external;

pub trait FormatRule {
    fn name(&self) -> &'static str;
    fn apply(
        &self,
        doc: Document,
        problem_config: ProblemConfig,
    ) -> Result<(Document, ProblemConfig)>;
}
