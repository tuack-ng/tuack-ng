use crate::prelude::*;
use crate::tuack_lib::config::CONFIG_VERSION;
use crate::tuack_lib::config::migrate::base::Migrater;
use crate::tuack_lib::config::migrate::v3::V3Migrater;
use bytesize::ByteSize;
use indexmap::IndexMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProblemConfigFile {
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
    /// 数据生成行为
    pub dmk: DmkConfig,
    /// 数据点参数 (全局部分)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,

    /// 样例
    pub samples: Vec<SampleItem>,
    /// 数据 (原始)
    pub data: Vec<DataItem>,
    /// Subtask 配置 (原始)
    #[serde(default, rename = "subtasks")]
    pub subtasks: BTreeMap<u32, ScorePolicy>,
    /// 测试用例
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub tests: IndexMap<String, TestCase>,
    /// 是否有 SPJ
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_chk: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProblemConfig {
    /// 配置文件版本，应至少以 `3` 开始
    /// 降低版本可能会引起迁移
    pub version: u32,
    /// 文件夹类型，在此处应为 `problem`
    pub folder: String,
    /// 题目类型
    pub problem_type: ProblemType,
    /// 题目 (英文) 名称
    pub name: String,
    /// 题目标题
    pub title: String,
    /// 时间限制
    pub time_limit: f64,
    /// 空间限制
    pub memory_limit: ByteSize,
    /// 是否有部分分，目前没有用途
    pub partial_score: bool,
    /// 数据生成行为
    pub dmk: DmkConfig,
    /// 数据点参数 (全局部分)
    pub args: HashMap<String, i64>,
    /// 样例
    pub samples: Vec<SampleItem>,
    /// 数据 (原始)
    pub orig_data: Vec<DataItem>,
    /// Subtask 配置 (原始)
    pub orig_subtasks: BTreeMap<u32, ScorePolicy>,
    /// 测试用例
    pub tests: IndexMap<String, TestCase>,
    /// 是否有 SPJ
    pub use_chk: Option<bool>,

    /// 是否有 pretest，目前没有用途
    pub use_pretest: Option<bool>,
    /// 是否是 NOI 风格
    pub noi_style: Option<bool>,
    /// 是否使用文件 IO
    pub file_io: Option<bool>,

    /// 当前配置所在路径，运行时生成
    pub path: PathBuf,
    /// 数据
    #[serde(skip_deserializing, default)] // 这玩意反序列化无意义 (TODO)?
    pub data: Vec<Arc<ExpandedDataItem>>,
    /// Subtask 配置
    #[serde(skip_deserializing, default)] // 同上
    pub subtasks: BTreeMap<u32, SubtaskItem>,
}

impl From<ProblemConfig> for ProblemConfigFile {
    fn from(config: ProblemConfig) -> Self {
        ProblemConfigFile {
            version: config.version,
            folder: config.folder,
            problem_type: config.problem_type,
            name: config.name,
            title: config.title,
            time_limit: config.time_limit,
            memory_limit: config.memory_limit,
            partial_score: config.partial_score,
            dmk: config.dmk,
            args: config.args,
            samples: config.samples,
            data: config.orig_data,
            subtasks: config.orig_subtasks,
            tests: config.tests,
            use_chk: config.use_chk,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DmkConfig {
    /// 忽略
    Skip,
    /// 只生成输入
    Input,
    /// 只生成输出
    Output,
    /// 启用
    On,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleItem {
    /// 样例编号
    pub id: u32,
    /// 输入文件
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// 输出文件
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// 参数，会从全局参数继承
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<DmkConfig>,
}

impl SampleItem {
    pub fn input_path(&self) -> String {
        self.input
            .clone()
            .unwrap_or_else(|| format!("{}.in", self.id))
    }
    pub fn output_path(&self) -> String {
        self.output
            .clone()
            .unwrap_or_else(|| format!("{}.ans", self.id))
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
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<DmkConfig>,
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
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<DmkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// 数据生成行为
    pub dmk: DmkConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProblemType {
    /// 传统型
    Program,
    /// 提交答案型
    Output,
    /// 交互型
    Interactive,
}

/// 加载题目配置
pub fn load_problem_config(problemconfig_path: &Path) -> Result<ProblemConfig> {
    // 读取并验证问题配置文件
    let problem_content = fs::read_to_string(problemconfig_path)?;
    let mut problem_json_value: serde_json::Value = serde_json::from_str(&problem_content)?;

    let version = problem_json_value
        .get("version")
        .and_then(|v| v.as_u64())
        .context("配置文件缺少版本号")?;

    // 检查版本
    if version < 3 {
        msg_error!(
            "配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。"
        );
        bail!("配置文件版本过低");
    }

    if version > CONFIG_VERSION {
        msg_error!("配置文件版本过高，可能是新版本的配置文件。请检查是否有新版本。");
        bail!("配置文件版本过高");
    }

    if version == 3 {
        info!("正在迁移 V3 题目配置文件");
        problem_json_value = V3Migrater::migrate_problem(problem_json_value)?;
    }

    // 先解析为 File 结构
    let problemconfig: ProblemConfigFile = serde_json::from_value(problem_json_value)?;

    let mut expand_data: Vec<Arc<ExpandedDataItem>> = vec![];

    for data in &problemconfig.data {
        match data {
            DataItem::Single(item) => {
                expand_data.push(Arc::new(ExpandedDataItem {
                    id: item.id,
                    score: item.score,
                    subtask: item.subtask,
                    input: item
                        .input
                        .clone()
                        .unwrap_or_else(|| format!("{}.in", item.id)),
                    output: item
                        .output
                        .clone()
                        .unwrap_or_else(|| format!("{}.ans", item.id)),
                    args: item.args.clone(),
                    dmk: item.dmk.unwrap_or(problemconfig.dmk),
                }));
            }
            DataItem::Bundle(item) => {
                for id in &item.id {
                    expand_data.push(Arc::new(ExpandedDataItem {
                        id: *id as u32,
                        score: item.score,
                        subtask: item.subtask,
                        input: format!("{}.in", id),
                        output: format!("{}.ans", id),
                        args: item.args.clone(),
                        dmk: item.dmk.unwrap_or(problemconfig.dmk),
                    }));
                }
            }
        }
    }

    let mut expand_subtasks: BTreeMap<u32, SubtaskItem> = problemconfig
        .subtasks
        .iter()
        .map(|(&id, &policy)| {
            (
                id,
                SubtaskItem {
                    items: Vec::new(),
                    max_score: 0,
                    policy,
                },
            )
        })
        .collect();

    // 将数据点分配到对应的 subtask
    for data in &expand_data {
        let subtask_id = data.subtask;
        if let Some(subtask) = expand_subtasks.get_mut(&subtask_id) {
            subtask.items.push(data.clone());
        } else {
            msg_warn!("无效的 Subtask ID #{}", subtask_id);
        }
    }

    // 计算每个 subtask 的最大分值
    for subtask in expand_subtasks.values_mut() {
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
        };
    }

    Ok(ProblemConfig {
        version: problemconfig.version,
        folder: problemconfig.folder,
        problem_type: problemconfig.problem_type,
        name: problemconfig.name,
        title: problemconfig.title,
        time_limit: problemconfig.time_limit,
        memory_limit: problemconfig.memory_limit,
        partial_score: problemconfig.partial_score,
        dmk: problemconfig.dmk,
        args: problemconfig.args,
        samples: problemconfig.samples,
        orig_data: problemconfig.data,
        orig_subtasks: problemconfig.subtasks,
        tests: problemconfig.tests,
        use_chk: problemconfig.use_chk,
        use_pretest: None,
        noi_style: None,
        file_io: None,
        path: problemconfig_path
            .parent()
            .map(|p| p.to_path_buf())
            .context("无法获取配置文件父目录")?,
        data: expand_data,
        subtasks: expand_subtasks,
    })
}

/// 将题目配置序列化为JSON字符串
pub fn save_problem_config(config: &ProblemConfig) -> Result<String> {
    let config_file: ProblemConfigFile = config.clone().into();
    let json = serde_json::to_string_pretty(&config_file)?;
    Ok(json)
}
