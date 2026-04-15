use crate::context::{CurrentLocation, gctx};
use crate::prelude::*;
use crate::tuack_lib::config::ExpandedDataItem;
use crate::tuack_lib::dmk::DmkStatus;
use crate::tuack_lib::dmk::gen_data;
use clap::Args;
use clap::ValueEnum;
use indicatif::ProgressBar;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Target {
    /// 正式测试数据
    Data,
    /// 样例数据
    Sample,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Data => write!(f, "data"),
            Target::Sample => write!(f, "sample"),
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum DmkCommand {
    /// 生成（未生成的）数据
    Gen,
    /// 重新生成数据（使用相同种子）
    Regen,
    /// 重置种子
    Reset,
}

#[derive(Args, Debug)]
#[command(version)]
pub struct DmkArgs {
    /// 目标类型
    #[arg(value_enum)]
    pub target: Target,

    /// 操作
    #[arg(value_enum)]
    pub action: DmkCommand,

    /// 操作对象，使用 `,` 和 `-` 分割 (如 1,2-3,4-10)
    #[arg(default_value = "all")]
    object: String,
}

/// 从字符串解析测试点，返回匹配的 ExpandedDataItem 列表
pub fn parse_test_object(
    s: &str,
    all_items: &[Arc<ExpandedDataItem>],
) -> Result<Vec<Arc<ExpandedDataItem>>> {
    let s = s.trim().to_lowercase();

    if s == "all" {
        return Ok(all_items.to_vec());
    }

    let mut result = Vec::new();
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        if let Some(pos) = part.find('-') {
            let start_str = &part[..pos];
            let end_str = &part[pos + 1..];

            let start = start_str
                .parse::<u32>()
                .with_context(|| format!("无效的起始ID: {}", start_str))?;
            let end = end_str
                .parse::<u32>()
                .with_context(|| anyhow!("无效的结束ID: {}", end_str))?;

            if start > end {
                bail!("起始ID不能大于结束ID: {}", part);
            }

            // 遍历查找在范围内的测试点
            for item in all_items.iter() {
                if item.id >= start && item.id <= end {
                    result.push(item.clone());
                }
            }
        } else {
            let id = part
                .parse::<u32>()
                .with_context(|| anyhow!("无效的测试点ID: {}", part))?;

            // 遍历查找匹配的测试点
            if let Some(item) = all_items.iter().find(|item| item.id == id) {
                result.push(item.clone());
            }
        }
    }

    Ok(result)
}

use crate::tuack_lib::dmk as tuack_dmk;

// 转换 Target
impl From<Target> for tuack_dmk::Target {
    fn from(value: Target) -> Self {
        match value {
            Target::Data => tuack_dmk::Target::Data,
            Target::Sample => tuack_dmk::Target::Sample,
        }
    }
}

// 转换 DmkCommand
impl From<DmkCommand> for tuack_dmk::DmkCommand {
    fn from(value: DmkCommand) -> Self {
        match value {
            DmkCommand::Gen => tuack_dmk::DmkCommand::Gen,
            DmkCommand::Regen => tuack_dmk::DmkCommand::Regen,
            DmkCommand::Reset => tuack_dmk::DmkCommand::Reset,
        }
    }
}

pub async fn main(args: DmkArgs) -> Result<()> {
    let config = gctx().config.as_ref().context("没有找到有效的工程")?;

    let (current_problem, current_day) =
        if let CurrentLocation::Problem(ref day, ref prog) = config.1 {
            let day_config = config
                .0
                .subconfig
                .get(day)
                .context(format!("无法获取天配置: {}", day))?;

            let problem_config = day_config
                .subconfig
                .get(prog)
                .context(format!("无法获取题目配置: {}/{}", day, prog))?;

            (problem_config, day_config)
        } else {
            bail!("本命令只能在题目目录下执行");
        };

    let data_items: Vec<Arc<ExpandedDataItem>> = match &args.target {
        Target::Data => current_problem.data.to_vec(),
        Target::Sample => current_problem
            .samples
            .iter()
            .map(|item| {
                Arc::new(ExpandedDataItem {
                    id: item.id,
                    score: 0,
                    subtask: 0,
                    input: item.input.get().unwrap().clone(),
                    output: item.output.get().unwrap().clone(),
                    args: item.args.clone(),
                    manual: item.manual.unwrap_or(false),
                })
            })
            .collect(),
    };

    let data_items: Vec<Arc<ExpandedDataItem>> =
        data_items.into_iter().filter(|item| !item.manual).collect();

    let (tx, mut rx) = mpsc::channel::<DmkStatus>(10);

    let gen_handle = tokio::spawn(async move {
        gen_data(
            tx,
            &args.target.into(),
            &args.action.into(),
            &parse_test_object(&args.object, &data_items)?,
            current_problem,
            current_day,
        )
        .await
    });

    let std_compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());
    let dmk_compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());

    let dmk_pb = gctx().multiprogress.add(ProgressBar::new(0));

    while let Some(status) = rx.recv().await {
        match status {
            DmkStatus::CompilingDmk => {
                dmk_compile_pb.enable_steady_tick(Duration::from_millis(100));
                dmk_compile_pb.set_message("编译数据生成器");
            }
            DmkStatus::CompiledDmk => {
                dmk_compile_pb.finish_and_clear();
            }
            DmkStatus::CompilingStd => {
                std_compile_pb.enable_steady_tick(Duration::from_millis(100));
                std_compile_pb.set_message("编译标程");
            }
            DmkStatus::CompiledStd => {
                std_compile_pb.finish_and_clear();
            }
            DmkStatus::StartDmk(size) => {
                dmk_pb.set_style(
                    indicatif::ProgressStyle::default_bar()
                        .template("  [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                        .unwrap()
                        .progress_chars("=> "),
                );
                dmk_pb.set_length(size as u64);
            }
            DmkStatus::DmkInput { id, status, error } => {
                msg_item!(status, "测试点 {} {}", id.to_string().cyan(), "输入".bold());
                if let Some(e) = error {
                    msg_error!("{}", e);
                }
            }
            DmkStatus::DmkOutput { id, status, error } => {
                msg_item!(status, "测试点 {} {}", id.to_string().cyan(), "输出".bold());
                if let Some(e) = error {
                    msg_error!("{}", e);
                }
            }
            DmkStatus::DmkProgress(progress) => dmk_pb.set_position(progress as u64),
            DmkStatus::DmkStart(progress) => {
                dmk_pb.set_message(format!("生成数据点 #{}", progress))
            }
            DmkStatus::Completed => {
                dmk_pb.finish_with_message("数据生成完成！");
            }
        }
    }

    gen_handle.await?
}
