use crate::prelude::*;
use crate::utils::optional::Optional;
use bytesize::ByteSize;
use indexmap::IndexMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProblemConfig {
    /// 配置文件版本，应至少以 `3` 开始
    /// 降低版本可能会引起迁移
    pub version: u32,
    /// 文件夹类型，在此处应为 `problem`
    pub folder: String,
    /// 题目类型
    #[serde(rename = "type")]
    pub problem_type: ProblemType,
    /// 题目 (英文) 名称
    pub name: String,
    /// 题目标题
    pub title: String,
    /// 时间限制
    #[serde(rename = "time limit")]
    pub time_limit: f64,
    /// 空间限制
    #[serde(rename = "memory limit")]
    pub memory_limit: ByteSize,
    /// 是否有部分分，目前没有用途
    #[serde(rename = "partial score")]
    pub partial_score: bool,
    /// 数据点参数 (全局部分)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    /// 样例
    pub samples: Vec<SampleItem>,
    /// 数据 (原始)
    #[serde(rename = "data")]
    pub orig_data: Vec<DataItem>,
    /// Subtask 配置 (原始)
    #[serde(default, rename = "subtasks")]
    pub orig_subtasks: BTreeMap<u32, ScorePolicy>,
    /// 测试用例
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub tests: IndexMap<String, TestCase>,
    /// 是否有 SPJ
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_chk: Option<bool>,

    /// 是否有 pretest，目前没有用途
    #[serde(default, skip)]
    pub use_pretest: Option<bool>,
    /// 是否是 NOI 风格
    #[serde(default, skip)]
    pub noi_style: Option<bool>,
    /// 是否使用文件 IO
    #[serde(default, skip)]
    pub file_io: Option<bool>,

    /// 当前配置所在路径，运行时生成
    #[serde(skip)]
    pub path: PathBuf,
    // pretest 还没加
    // pub pretest: Vec<PreItem>,
    /// 数据
    #[serde(skip, default)]
    pub data: Vec<Arc<ExpandedDataItem>>,
    /// Subtask 配置
    #[serde(skip, default)]
    pub subtasks: BTreeMap<u32, SubtaskItem>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProblemType {
    /// 传统型
    Program,
    /// 提交答案型
    Output,
    /// 交互型
    Interactive,
}

#[derive(Debug, Clone)]
pub struct SubtaskItem {
    /// 数据点
    pub items: Vec<Arc<ExpandedDataItem>>,
    /// 最大分值
    pub max_score: u32,
    /// 评分策略
    pub policy: ScorePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// 期望得分条件
    pub expected: ExpectedScore,
    /// 文件或文件夹路径
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpectedScore {
    /// 单个条件，如 `">= 60"`
    Single(String),
    /// 多个条件，如 `[">= 60", "< 90"]`
    Multiple(Vec<String>),
}

impl ProblemConfig {
    pub fn finalize(mut self) -> Self {
        for data in &self.orig_data {
            match data {
                DataItem::Single(item) => self.data.push(Arc::new(ExpandedDataItem {
                    id: item.id,
                    score: item.score,
                    subtask: item.subtask,
                    input: item
                        .input
                        .clone()
                        .unwrap_or(format!("{}.in", item.id))
                        .clone(),
                    output: item
                        .output
                        .clone()
                        .unwrap_or(format!("{}.ans", item.id))
                        .clone(),
                    args: item.args.clone(),
                    manual: item.manual.unwrap_or(false),
                })),
                DataItem::Bundle(item) => {
                    for id in &item.id {
                        self.data.push(Arc::new(ExpandedDataItem {
                            id: *id as u32,
                            score: item.score,
                            subtask: item.subtask,
                            input: format!("{}.in", id),
                            output: format!("{}.ans", id),
                            args: item.args.clone(),
                            manual: item.manual.unwrap_or(false),
                        }))
                    }
                }
            }
        }

        self.subtasks = self
            .orig_subtasks
            .iter()
            .map(|item| {
                (
                    *item.0,
                    SubtaskItem {
                        items: Vec::new(),
                        max_score: 0,
                        policy: *item.1,
                    },
                )
            })
            .collect();

        for data in &self.data {
            let subtask_id = data.subtask;
            if !self.subtasks.contains_key(&subtask_id) {
                msg_warn!("无效的 Subtask ID #{}", &subtask_id);
                continue;
            }
            self.subtasks
                .get_mut(&subtask_id)
                .unwrap()
                .items
                .push(data.clone());
        }

        for subtask in self.subtasks.values_mut() {
            subtask.max_score = match subtask.policy {
                ScorePolicy::Max => subtask
                    .items
                    .iter()
                    .map(|item| item.score)
                    .max()
                    .unwrap_or(0),
                ScorePolicy::Min => subtask
                    .items
                    .iter()
                    .map(|item| item.score)
                    .min()
                    .unwrap_or(0),
                ScorePolicy::Sum => subtask.items.iter().map(|item| item.score).sum(),
            }
        }

        // 初始化 samples 的默认文件名
        self.samples = self.samples.into_iter().map(|s| s.finalize()).collect();

        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleItem {
    /// 样例编号
    pub id: u32,
    /// 输入文件
    #[serde(
        skip_serializing_if = "Optional::should_skip",
        default = "Optional::uninitialized"
    )]
    pub input: Optional<String>,
    /// 输出文件
    #[serde(
        skip_serializing_if = "Optional::should_skip",
        default = "Optional::uninitialized"
    )]
    pub output: Optional<String>,
    /// 参数，会从全局参数继承
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    /// 是否为人工生成的测试点
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
    /// 单个对象
    Single(SingleDataItem),
    /// 组合对象
    Bundle(BundleDataItem),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleDataItem {
    /// 测试点编号
    pub id: u32,
    /// 测试点分值
    pub score: u32,
    /// Subtask 编号
    #[serde(default)]
    pub subtask: u32,
    /// 输入文件
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub input: Option<String>,
    /// 输出文件
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output: Option<String>,
    /// 参数，会从全局参数继承
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    /// 是否为人工生成的测试点
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleDataItem {
    /// 测试点编号
    pub id: Vec<i32>,
    /// 测试点分值
    pub score: u32,
    /// Subtask 编号
    #[serde(default)]
    pub subtask: u32,
    /// 参数，会从全局参数继承
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    /// 是否为人工生成的测试点
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ExpandedDataItem {
    /// 测试点编号
    pub id: u32,
    /// 测试点分值
    pub score: u32,
    /// Subtask 编号
    pub subtask: u32,
    /// 输入文件
    pub input: String,
    /// 输出文件
    pub output: String,
    /// 参数，会从全局参数继承
    pub args: HashMap<String, i64>,
    /// 是否为人工生成的测试点
    pub manual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
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
        msg_error!(
            "配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。"
        );
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
