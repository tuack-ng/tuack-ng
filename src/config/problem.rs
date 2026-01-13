use log::error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
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
    #[serde(default)]
    pub subtests: BTreeMap<u32, ScorePolicy>,
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
    pub subtest: u32,
    #[serde(skip)]
    pub input: Option<String>,
    #[serde(skip)]
    pub output: Option<String>,
}

impl DataItem {
    pub fn finalize(self) -> Self {
        // 总是使用默认的 id+.in/.ans 格式，忽略配置文件中的设置
        DataItem {
            id: self.id,
            score: self.score,
            subtest: self.subtest,
            input: Some(format!("{}.in", self.id)),
            output: Some(format!("{}.ans", self.id)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScorePolicy {
    /// 求和（默认）
    Sum,
    /// 求最大值
    Max,
    /// 求最小值
    Min,
}

/// 加载题目配置
pub fn load_problem_config(
    problemconfig_path: &Path,
) -> Result<ProblemConfig, Box<dyn std::error::Error>> {
    // 读取并验证问题配置文件
    let problem_content = fs::read_to_string(problemconfig_path)?;
    let problem_json_value: serde_json::Value = serde_json::from_str(&problem_content)?;

    // 检查版本
    if let Some(version) = problem_json_value.get("version").and_then(|v| v.as_u64())
        && version < 3
    {
        error!("配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。");
        return Err("配置文件版本过低".into());
    }

    let mut problemconfig: ProblemConfig = serde_json::from_str(&problem_content)?;

    problemconfig.path = problemconfig_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or("无法获取配置文件父目录")?;

    problemconfig = problemconfig.finalize();

    Ok(problemconfig)
}

/// 将题目配置序列化为JSON字符串，排除null字段
pub fn save_problem_config(config: &ProblemConfig) -> Result<String, Box<dyn std::error::Error>> {
    let json_value = serde_json::to_value(config)?;
    let filtered_obj = json_value
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<serde_json::Map<_, _>>()
        })
        .ok_or("Failed to convert problem config to object")?;
    let json = serde_json::to_string_pretty(&filtered_obj)?;
    Ok(json)
}
