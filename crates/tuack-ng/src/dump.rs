use crate::prelude::*;
use clap::Args;
use clap::ValueEnum;
use std::collections::HashSet;
use std::time::Duration;
use tuack_lib::dump::{
    DumpCase, DumpConfig, DumpDocument, DumpFile, DumpProblem, DumpSample, DumpSubtask, Dumper,
    ScorePolicy,
};
use tuack_lib::ren::ProblemType;
use tuack_utils::assets::FsAssetProvider;
use tuack_utils::dump::{arbiter, lemon};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Target {
    Lemon,
    Arbiter,
}

impl Target {
    /// 输出子目录名
    pub fn as_str(&self) -> &'static str {
        match self {
            Target::Lemon => "lemon",
            Target::Arbiter => "arbiter",
        }
    }
}

#[derive(Args, Debug)]
#[command(version)]
pub struct DumpArgs {
    /// 导出目标
    #[arg(required = true)]
    pub target: Target,
}

/// 递归枚举 down/ 目录的非样例附加文件
/// `rel` 为相对 down 根的路径，样例排除用"相对各自资源根的路径"比较。
fn collect_extra_down(
    dir: &Path,
    rel: &str,
    sample_files: &HashSet<String>,
    out: &mut Vec<DumpFile>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", rel, name)
        };
        let ftype = entry.file_type()?;
        if ftype.is_dir() {
            collect_extra_down(&entry.path(), &child_rel, sample_files, out)?;
        } else if ftype.is_file() && !sample_files.contains(&child_rel) {
            out.push(DumpFile {
                path: PathBuf::from(format!("down/{}", child_rel)),
            });
        }
    }
    Ok(())
}

/// 从 config 提取 day 级导出文档（前端构造纯数据，dumper 不接触 config 类型）。
fn build_dump_document(
    contest: &ContestConfig,
    day: &ContestDayConfig,
    daynum: usize,
) -> Result<DumpDocument> {
    let compile = day
        .compile
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let mut assets = FsAssetProvider::new();
    let mut problems = Vec::new();
    for (idx, (_, prob)) in day.subconfig.iter().enumerate() {
        assets.register(idx as u64, prob.path.clone());

        let data = prob
            .runtime
            .data
            .iter()
            .map(|case| DumpCase {
                id: case.id,
                score: case.score,
                subtask: case.subtask,
                input: PathBuf::from(format!("data/{}", case.input)),
                output: PathBuf::from(format!("data/{}", case.output)),
            })
            .collect();

        let subtasks = prob
            .runtime
            .subtasks
            .iter()
            .map(|(k, st)| {
                (
                    *k,
                    DumpSubtask {
                        items: st.items.clone(),
                        max_score: st.max_score,
                        policy: match st.policy {
                            tuack_config::ScorePolicy::Sum => ScorePolicy::Sum,
                            tuack_config::ScorePolicy::Min => ScorePolicy::Min,
                            tuack_config::ScorePolicy::Max => ScorePolicy::Max,
                        },
                    },
                )
            })
            .collect();

        let samples = prob
            .samples
            .iter()
            .map(|s| DumpSample {
                input: PathBuf::from(format!("sample/{}", s.input_path())),
                output: PathBuf::from(format!("sample/{}", s.output_path())),
            })
            .collect();

        let checker = prob.checker.as_ref().map(|c| PathBuf::from(&c.data.source));

        let mut extra_down = Vec::new();
        let extra_dir = prob.path.join("down");
        if extra_dir.exists() {
            let sample_files: HashSet<String> = prob
                .samples
                .iter()
                .flat_map(|s| [s.input_path(), s.output_path()])
                .collect();
            collect_extra_down(&extra_dir, "", &sample_files, &mut extra_down)?;
        }

        problems.push(DumpProblem {
            idx: idx as u64,
            name: prob.name.clone(),
            title: prob.title.clone(),
            problem_type: match prob.problem_type {
                tuack_config::ProblemType::Program => ProblemType::Program,
                tuack_config::ProblemType::Output => ProblemType::Output,
                tuack_config::ProblemType::Interactive => ProblemType::Interactive,
            },
            time_limit: Duration::from_secs_f64(prob.time_limit),
            memory_limit: prob.memory_limit,
            data,
            subtasks,
            samples,
            extra_down,
            checker,
        });
    }

    Ok(DumpDocument {
        config: DumpConfig {
            contest_name: contest.name.clone(),
            day_name: day.name.clone(),
            dayidx: daynum,
            compile,
        },
        problems,
        assets: Box::new(assets),
    })
}

async fn dump_main(
    contest: &ContestConfig,
    day: &ContestDayConfig,
    daynum: usize,
    target: Target,
) -> Result<()> {
    let doc = build_dump_document(contest, day, daynum)?;
    let dump_dir = day.path.join("dump");

    let tmp = tempfile::Builder::new()
        .prefix("tuack-ng-dump-")
        .tempdir()
        .context("创建临时目录失败")?;

    let dumper: Box<dyn Dumper> = match target {
        Target::Lemon => Box::new(lemon::LemonDumper::new(tmp.path().to_path_buf())),
        Target::Arbiter => Box::new(arbiter::ArbiterDumper::new(
            tmp.path().to_path_buf(),
            gctx().assets_dirs.clone(),
        )),
    };

    let (files, warnings) = match dumper.dump(&doc).await {
        Ok(result) => result,
        Err(e) => {
            msg_error!("导出失败:\n{:?}", e);
            let kept = tmp.keep();
            msg_info!("保留临时目录以供调试：{}", kept.display());
            bail!("导出过程出错");
        }
    };

    for warning in &warnings {
        msg_warn!("{}", warning);
    }

    let dir_name = target.as_str();
    let out_dir = dump_dir.join(dir_name);
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir)?;
    }

    if let Err(e) = crate::utils::filesystem::write_outputs(&dump_dir, files).await {
        msg_error!("写入导出结果失败：{:?}", e);
        let kept = tmp.keep();
        msg_info!("保留临时目录以供调试：{}", kept.display());
        bail!("写入导出结果失败");
    }
    msg_info!("导出完成，输出目录：{}", out_dir.display());

    Ok(())
}

pub async fn main(args: DumpArgs) -> Result<()> {
    if gctx().config.is_none() {
        bail!("没有有效的配置文件");
    }
    let config = gctx().config.clone().unwrap();
    match config.location {
        CurrentLocation::None => bail!("此命令必须在工程下执行"),
        CurrentLocation::Problem(_, _) => bail!("此命令不能在题目下执行"),
        CurrentLocation::Day(day) => {
            dump_main(
                &config.config,
                config.config.subconfig.get(&day).unwrap(),
                1,
                args.target,
            )
            .await?;
        }
        CurrentLocation::Root => {
            for (idx, (_, day_config)) in config.config.subconfig.iter().enumerate() {
                dump_main(&config.config, day_config, idx + 1, args.target).await?;
            }
        }
    }

    Ok(())
}
