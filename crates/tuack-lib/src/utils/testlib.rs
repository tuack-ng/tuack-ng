use async_trait::async_trait;

use crate::data::AsyncReader;
use crate::prelude::*;

/// 数据生成器参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    Integer(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

/// 数据生成器
#[async_trait]
pub trait Generator: Send {
    fn prepare(&mut self) -> Result<()>;
    /// 运行生成器，返回生成的输入流
    async fn run(&self, args: IndexMap<String, Arg>, seed: u64) -> Result<Box<dyn AsyncReader>>;
}

/// Checker（SPJ）结果类型
#[derive(Debug, Clone, PartialEq)]
pub enum JudgeResult {
    Accepted,
    WrongAnswer,
    PresentationError,
    Fail,
    Score(f64),
}

/// Checker（SPJ）
#[async_trait]
pub trait Checker: Send {
    fn prepare(&mut self) -> Result<()>;
    async fn validate(
        &self,
        input: &mut dyn AsyncReader,
        output: &mut dyn AsyncReader,
        answer: &mut dyn AsyncReader,
    ) -> Result<(JudgeResult, String)>;
}

/// Validator（输入校验器）结果
#[derive(Debug, Clone, PartialEq)]
pub enum ValidatorResult {
    /// 校验通过
    Ok,
    /// 校验失败，附带 stderr 信息
    Invalid(String),
}

/// Validator（输入校验器）
#[async_trait]
pub trait Validator: Send {
    fn prepare(&mut self) -> Result<()>;
    async fn validate(&self, input: &mut dyn AsyncReader) -> Result<ValidatorResult>;
}
