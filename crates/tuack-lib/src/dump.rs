//! Dump 后端：导出抽象，与 ren 同构。
//!
//! - `DumpDocument` 是不可变输入（day 级纯数据），`Dumper::dump` 产出 `(Vec<OutputFile>, Vec<String>)`——
//!   导出产物文件与导出过程中的面向用户警告（如平台限制、编译失败提示），由调用方负责展示。
//! - dumper 不访问配置对象；用户资源（data/sample/down/checker）一律经 `AssetProvider` 获取。
//! - dumper 可进行为生成导出产物所必需的内部 I/O（写临时目录、编译 checker 等）

use bytesize::ByteSize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::prelude::*;
use crate::ren::ProblemType;
use crate::utils::asset::AssetProvider;
use crate::utils::output::OutputFile;

/// 评分策略（渲染后端无关枚举）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScorePolicy {
    Sum,
    Min,
    Max,
}

/// day 级导出配置
#[derive(Debug, Clone)]
pub struct DumpConfig {
    pub contest_name: String,
    pub day_name: String,
    pub dayidx: usize,
    /// 编译选项（语言，选项）
    pub compile: Vec<(String, String)>,
}

/// 单个测试点
#[derive(Debug, Clone)]
pub struct DumpCase {
    pub id: u32,
    pub score: u32,
    pub subtask: u32,
    /// 输入相对路径（如 `data/1.in`）
    pub input: PathBuf,
    /// 输出相对路径（如 `data/1.ans`）
    pub output: PathBuf,
}

/// Subtask
#[derive(Debug, Clone)]
pub struct DumpSubtask {
    /// 数据点在 data 中的下标
    pub items: Vec<usize>,
    pub max_score: u32,
    pub policy: ScorePolicy,
}

/// 样例（相对路径，如 `sample/a.in`）
#[derive(Debug, Clone)]
pub struct DumpSample {
    pub input: PathBuf,
    pub output: PathBuf,
}

/// 一个待导出的文件，相对题目根的逻辑路径
#[derive(Debug, Clone)]
pub struct DumpFile {
    pub path: PathBuf,
}

/// 单题导出数据
#[derive(Debug, Clone)]
pub struct DumpProblem {
    /// 题目编号（与 assets 登记一致）
    pub idx: u64,
    pub name: String,
    pub title: String,
    pub problem_type: ProblemType,
    pub time_limit: Duration,
    pub memory_limit: ByteSize,
    pub data: Vec<DumpCase>,
    pub subtasks: BTreeMap<u32, DumpSubtask>,
    pub samples: Vec<DumpSample>,
    /// down/ 下非样例的附加文件
    pub extra_down: Vec<DumpFile>,
    /// checker 源文件逻辑路径
    pub checker: Option<PathBuf>,
}

/// 导出文档：dumper 的唯一输入（day 级）。
pub struct DumpDocument {
    pub config: DumpConfig,
    pub problems: Vec<DumpProblem>,
    /// 资源 handle：按 `(题目编号，相对路径)` 惰性返回数据/样例流
    pub assets: Box<dyn AssetProvider>,
}

/// 导出器：`DumpDocument -> (产物文件列表, 导出警告)`。
#[async_trait]
pub trait Dumper: Send + Sync {
    async fn dump(&self, doc: &DumpDocument) -> Result<(Vec<OutputFile>, Vec<String>)>;
}
