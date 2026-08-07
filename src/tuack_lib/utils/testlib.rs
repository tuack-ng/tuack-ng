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
pub trait Generator: Send {
    fn prepare(&mut self) -> Result<()>;
    fn run(&self, args: IndexMap<String, Arg>, seed: u64) -> Result<Vec<u8>>;
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
pub trait Checker: Send {
    fn prepare(&mut self) -> Result<()>;
    fn validate(&self, input: &Path, output: &[u8], answer: &Path)
    -> Result<(JudgeResult, String)>;
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
pub trait Validator: Send {
    fn prepare(&mut self) -> Result<()>;
    fn validate(&self, input: &[u8]) -> Result<ValidatorResult>;
}
