use bytesize::ByteSize;
use std::time::Duration;
use tuack_ng_parser::ast::Document;

use crate::prelude::*;

/// 题目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProblemType {
    Program,
    Output,
    Interactive,
}

/// 题目渲染元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemMeta {
    /// 题目 (英文) 名称
    pub name: String,
    pub title: String,
    pub problem_type: ProblemType,
    pub time_limit: Duration,
    pub memory_limit: ByteSize,
    pub testcase: usize,
    /// 各测试点分数是否相等
    pub point_equal: bool,
    /// 可提交的文件名（如 ["a.cpp", "a.py"]）
    pub submit_filename: Vec<String>,
}

/// 支持的语言
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportLanguage {
    pub name: String,
    pub compile_options: String,
}

/// 比赛日起止时间
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateInfo {
    pub start: [u32; 6],
    pub end: [u32; 6],
}

/// 渲染配置，包含渲染所需的全部信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenConfig {
    pub title: String,
    pub short_title: String,
    pub day_key: String,
    pub dayname: String,
    pub date: Option<DateInfo>,
    pub use_pretest: bool,
    pub noi_style: bool,
    pub file_io: bool,
    pub support_languages: Vec<SupportLanguage>,
}

/// 一道题的完整渲染数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    /// 题目编号（0 起）
    pub idx: u64,
    pub meta: ProblemMeta,
    pub ast: Document,
    /// 图片重写映射：原始 URL -> 目标 URL（题目层级，避免跨题重名）。
    pub images: IndexMap<PathBuf, PathBuf>,
}

/// 渲染文档，渲染器的输入。
///
/// 资源访问（`AssetProvider`）作为能力在 `Renderer::render` 时单独注入，
/// 不随文档数据传递，因此本结构可序列化、可跨边界（如 WASM 插件）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderDocument {
    pub config: RenConfig,
    pub problems: Vec<Problem>,
    pub precaution: Option<Document>,
}
