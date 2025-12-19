use serde::{Deserialize, Serialize};
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
    // pub tests: HashMap<String, serde_json::Value>,
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
    pub fn finalize(mut self) -> Self {
        if self.input.as_ref().map_or(true, |s| s.is_empty()) {
            self.input = Some(format!("{}.in", self.id));
        }
        if self.output.as_ref().map_or(true, |s| s.is_empty()) {
            self.output = Some(format!("{}.ans", self.id));
        }
        self
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
    pub fn finalize(mut self) -> Self {
        if self.input.as_ref().map_or(true, |s| s.is_empty()) {
            self.input = Some(format!("{}.in", self.id));
        }
        if self.output.as_ref().map_or(true, |s| s.is_empty()) {
            self.output = Some(format!("{}.ans", self.id));
        }
        self
    }
}
