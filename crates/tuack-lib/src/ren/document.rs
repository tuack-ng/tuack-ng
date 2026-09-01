use bytesize::ByteSize;
use std::time::Duration;
use tuack_ng_parser::ast::Document;

use crate::utils::asset::AssetProvider;

/// 题目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemType {
    Program,
    Output,
    Interactive,
}

/// 题目渲染元信息
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct SupportLanguage {
    pub name: String,
    pub compile_options: String,
}

/// 比赛日起止时间
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateInfo {
    pub start: [u32; 6],
    pub end: [u32; 6],
}

/// 渲染配置，包含渲染所需的全部信息。
#[derive(Debug, Clone)]
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
#[derive(Debug)]
pub struct Problem {
    /// 题目编号（0 起）
    pub idx: u64,
    pub meta: ProblemMeta,
    pub ast: Document,
}

/// 渲染文档，渲染器的输入。
pub struct RenderDocument {
    pub config: RenConfig,
    pub problems: Vec<Problem>,
    pub precaution: Option<Document>,
    pub assets: Box<dyn AssetProvider>,
}
