use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProblemConfig {
    pub version: u32,
    pub folder: String,
    #[serde(rename = "type")]
    pub problem_type: String,
    pub name: String,
    pub title: String,
    #[serde(rename = "time limit")]
    pub time_limit: f64,
    #[serde(rename = "memory limit")]
    pub memory_limit: String,
    #[serde(rename = "partial score")]
    pub partial_score: bool,
    #[serde(skip)]
    pub path: PathBuf,
    pub samples: Vec<SampleItem>,
    // pub args: HashMap<String, serde_json::Value>,
    pub data: Vec<DataItem>,
    // pub pretest: Vec<PreItem>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tests: HashMap<String, TestCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub expected: ExpectedScore, // 期望得分条件
    pub path: String,            // 文件或文件夹路径
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpectedScore {
    Single(String),        // 单个条件，如 ">= 60"
    Multiple(Vec<String>), // 多个条件，如 [">= 60", "< 90"]
}

impl ProblemConfig {
    pub fn finalize(mut self) -> Self {
        // 初始化 samples 的默认文件名
        self.samples = self.samples.into_iter().map(|s| s.finalize()).collect();

        // 初始化 data 的默认文件名
        self.data = self.data.into_iter().map(|d| d.finalize()).collect();

        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleItem {
    pub id: u32,
    #[serde(default)]
    pub input: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
}

impl SampleItem {
    pub fn finalize(self) -> Self {
        // 总是使用默认的 id+.in/.ans 格式，忽略配置文件中的设置
        SampleItem {
            id: self.id,
            input: Some(format!("{}.in", self.id)),
            output: Some(format!("{}.ans", self.id)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataItem {
    pub id: u32,
    pub score: u32,
    #[serde(default)]
    pub input: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
}

impl DataItem {
    pub fn finalize(self) -> Self {
        // 总是使用默认的 id+.in/.ans 格式，忽略配置文件中的设置
        DataItem {
            id: self.id,
            score: self.score,
            input: Some(format!("{}.in", self.id)),
            output: Some(format!("{}.ans", self.id)),
        }
    }
}
