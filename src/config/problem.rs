use crate::prelude::*;
use crate::utils::optional::Optional;
use indexmap::IndexMap;

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
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    #[serde(rename = "data")]
    pub orig_data: Vec<DataItem>,
    #[serde(default)]
    pub subtasks: BTreeMap<u32, ScorePolicy>,
    // pub pretest: Vec<PreItem>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub tests: IndexMap<String, TestCase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_chk: Option<bool>,

    #[serde(default, skip, rename = "use-pretest")]
    pub use_pretest: Option<bool>,
    #[serde(default, skip, rename = "noi-style")]
    pub noi_style: Option<bool>,
    #[serde(default, skip, rename = "file-io")]
    pub file_io: Option<bool>,

    #[serde(skip, default)]
    pub data: Vec<ExpandedDataItem>,
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
        // 初始化 data 的默认文件名
        self.orig_data = self.orig_data.into_iter().map(|d| d.finalize()).collect();

        for data in &self.orig_data {
            match data {
                DataItem::Single(item) => self.data.push(ExpandedDataItem {
                    id: item.id,
                    score: item.score,
                    subtask: item.subtask,
                    input: item.input.get().unwrap().clone(),
                    output: item.output.get().unwrap().clone(),
                    args: item.args.clone(),
                    manual: item.manual.unwrap_or(false),
                }),
                DataItem::Bundle(item) => {
                    for id in &item.id {
                        self.data.push(ExpandedDataItem {
                            id: *id as u32,
                            score: item.score,
                            subtask: item.subtask,
                            input: format!("{}.in", id),
                            output: format!("{}.ans", id),
                            args: item.args.clone(),
                            manual: item.manual.unwrap_or(false),
                        })
                    }
                }
            }
        }

        // 初始化 samples 的默认文件名
        self.samples = self.samples.into_iter().map(|s| s.finalize()).collect();

        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleItem {
    pub id: u32,
    #[serde(
        skip_serializing_if = "Optional::should_skip",
        default = "Optional::uninitialized"
    )]
    pub input: Optional<String>,
    #[serde(
        skip_serializing_if = "Optional::should_skip",
        default = "Optional::uninitialized"
    )]
    pub output: Optional<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
}

impl SampleItem {
    pub fn finalize(mut self) -> Self {
        self.input.set_default(format!("{}.in", self.id));
        self.output.set_default(format!("{}.ans", self.id));
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DataItem {
    Single(SingleDataItem), // 单个条件，如 ">= 60"
    Bundle(BundleDataItem), // 多个条件，如 [">= 60", "< 90"]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleDataItem {
    pub id: u32,
    pub score: u32,
    #[serde(default)]
    pub subtask: u32,
    // #[serde(skip)]
    #[serde(
        skip_serializing_if = "Optional::should_skip",
        default = "Optional::uninitialized"
    )]
    pub input: Optional<String>,
    #[serde(
        skip_serializing_if = "Optional::should_skip",
        default = "Optional::uninitialized"
    )]
    pub output: Optional<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleDataItem {
    pub id: Vec<i32>,
    pub score: u32,
    #[serde(default)]
    pub subtask: u32,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ExpandedDataItem {
    pub id: u32,
    pub score: u32,
    pub subtask: u32,
    pub input: String,
    pub output: String,
    pub args: HashMap<String, i64>,
    pub manual: bool,
}

impl DataItem {
    pub fn finalize(mut self) -> Self {
        if let DataItem::Single(ref mut item) = self {
            item.input.set_default(format!("{}.in", item.id));
            item.output.set_default(format!("{}.ans", item.id));
        }
        self
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
pub fn load_problem_config(problemconfig_path: &Path) -> Result<ProblemConfig> {
    // 读取并验证问题配置文件
    let problem_content = fs::read_to_string(problemconfig_path)?;
    let problem_json_value: serde_json::Value = serde_json::from_str(&problem_content)?;

    // 检查版本
    if let Some(version) = problem_json_value.get("version").and_then(|v| v.as_u64())
        && version < 3
    {
        error!("配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。");
        bail!("配置文件版本过低");
    }

    let mut problemconfig: ProblemConfig = serde_json::from_str(&problem_content)?;

    problemconfig.path = problemconfig_path
        .parent()
        .map(|p| p.to_path_buf())
        .context("无法获取配置文件父目录")?;

    problemconfig = problemconfig.finalize();

    Ok(problemconfig)
}

/// 将题目配置序列化为JSON字符串，排除null字段
pub fn save_problem_config(config: &ProblemConfig) -> Result<String> {
    let json_value = serde_json::to_value(config)?;
    let filtered_obj = json_value
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<serde_json::Map<_, _>>()
        })
        .context("Failed to convert problem config to object")?;
    let json = serde_json::to_string_pretty(&filtered_obj)?;
    Ok(json)
}
