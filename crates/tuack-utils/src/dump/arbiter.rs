use std::process::Command;

use crate::prelude::*;
use tuack_lib::dump::Dumper;
use tuack_lib::ren::ProblemType;
use tuack_lib::utils::output::OutputFile;

/// 生成 key=value 配置文件内容
fn build_info(info: &[(String, String)]) -> String {
    let mut content = String::new();
    for (key, val) in info {
        content.push_str(key);
        content.push_str(val);
        content.push('\n');
    }
    content
}

pub struct ArbiterDumper {
    tmp_dir: PathBuf,
    assets_dirs: Vec<PathBuf>,
}

impl ArbiterDumper {
    pub fn new(tmp_dir: PathBuf, assets_dirs: Vec<PathBuf>) -> Self {
        Self {
            tmp_dir,
            assets_dirs,
        }
    }

    /// 生成 filter 可执行文件：有自定义 SPJ 则编译 checker（经 handle 取源码），
    /// 否则编译默认比较器源码；编译失败时 `None`（不产生文件），资源缺失则失败
    async fn build_filter(
        &self,
        doc: &tuack_lib::dump::DumpDocument,
        prob: &tuack_lib::dump::DumpProblem,
        warnings: &mut Vec<String>,
    ) -> Result<Option<Box<dyn tuack_lib::data::AsyncReader>>> {
        let filter_path = self.tmp_dir.join(format!("{}_e", prob.name));

        // 有自定义 SPJ：编译 checker
        if let Some(checker) = &prob.checker {
            info!("发现 chk，尝试编译。");
            let src_tmp = self.tmp_dir.join("chk-src.cpp");
            let mut src = doc.assets.load(prob.idx, &checker.source).await?;
            let mut f = tokio::fs::File::create(&src_tmp).await?;
            tokio::io::copy(&mut src, &mut f).await?;
            drop(f);

            for dep in &checker.deps {
                let mut dep_src = doc.assets.load(prob.idx, dep).await?;
                let dep_name = dep.file_name().context("依赖路径缺少文件名")?.to_owned();
                let dep_tmp = self.tmp_dir.join(&dep_name);
                let mut f = tokio::fs::File::create(&dep_tmp).await?;
                tokio::io::copy(&mut dep_src, &mut f).await?;
                drop(f);
            }

            let status = Command::new("g++")
                .arg(&src_tmp)
                .arg("-o")
                .arg(&filter_path)
                .arg("-O2")
                .arg("-std=c++17")
                .status()
                .context("执行 g++ 失败")?;

            if !status.success() {
                warnings.push(format!("chk 编译失败：{}", checker.source.display()));
                return Ok(None);
            }
            return Ok(Some(Box::new(tokio::fs::File::open(&filter_path).await?)));
        }

        // 无自定义 SPJ：编译默认比较器源码（跨平台，比预编译二进制更可维护）
        let default_src = self.assets_dirs.iter().find_map(|d| {
            let p = d.join("sample").join("default_arbiter.cpp");
            p.exists().then_some(p)
        });

        match default_src {
            Some(src) => {
                info!("编译默认比较器：{}", src.display());
                let status = Command::new("g++")
                    .arg(&src)
                    .arg("-o")
                    .arg(&filter_path)
                    .arg("-O2")
                    .arg("-std=c++17")
                    .status()
                    .context("执行 g++ 失败")?;
                if !status.success() {
                    warnings.push(format!("默认比较器编译失败：{}", src.display()));
                    return Ok(None);
                }
                Ok(Some(Box::new(tokio::fs::File::open(&filter_path).await?)))
            }
            None => {
                bail!(
                    "题目 {} 没有 chk，也未找到默认比较器源码 assets/sample/default_arbiter.cpp，无法生成 filter。",
                    prob.name
                );
            }
        }
    }
}

#[async_trait]
impl Dumper for ArbiterDumper {
    async fn dump(
        &self,
        doc: &tuack_lib::dump::DumpDocument,
    ) -> Result<(Vec<OutputFile>, Vec<String>)> {
        if !cfg!(target_os = "linux") {
            bail!("Arbiter 不支持 Linux 之外的操作系统，也不支持在 Linux 之外的操作系统导出");
        }

        let daynum = doc.config.dayidx;
        let mut files = Vec::new();
        let mut warnings = Vec::new();

        // Arbiter 要求的目录结构（可能没有文件，需确保存在）
        for sub in ["data", "final", "players", "result", "filter", "tmp"] {
            files.push(OutputFile::Dir(PathBuf::from(format!(
                "arbiter/main/{}",
                sub
            ))));
        }
        files.push(OutputFile::Dir(PathBuf::from(format!(
            "arbiter/main/players/day{}",
            daynum
        ))));
        files.push(OutputFile::Dir(PathBuf::from(format!(
            "arbiter/main/result/day{}",
            daynum
        ))));

        // 写 day{N}.info
        let dayinfo: Vec<(String, String)> = vec![
            ("NAME=".into(), format!("第{}场--机试", daynum)),
            ("PLAYERDIR=".into(), "".into()),
            ("CASEDIR=".into(), "".into()),
            ("BASESCORE=".into(), "0".into()),
            ("TASKNUM=".into(), doc.problems.len().to_string()),
        ];
        files.push(OutputFile::File {
            path: PathBuf::from(format!("arbiter/main/day{}.info", daynum)),
            bytes: Box::new(std::io::Cursor::new(build_info(&dayinfo).into_bytes())),
        });

        for (probnum, prob) in doc.problems.iter().enumerate() {
            let probnum = probnum + 1;
            info!("处理题目：{}", prob.name);

            let score_per_case = if prob.data.is_empty() {
                0u32
            } else {
                100 / prob.data.len() as u32
            };

            if !prob.data.is_empty()
                && prob.subtasks.len() <= 1
                && score_per_case * prob.data.len() as u32 != 100
            {
                warnings.push(format!(
                    "题目 {} 的测试点数量不是 100 的约数，分数无法均分为整数。",
                    prob.name
                ));
            }

            let c_args = doc
                .config
                .compile
                .iter()
                .find(|(k, _)| k == "c")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let cpp_args = doc
                .config
                .compile
                .iter()
                .find(|(k, _)| k == "cpp")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let pas_args = doc
                .config
                .compile
                .iter()
                .find(|(k, _)| k == "pas")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();

            let mut probinfo: Vec<(String, String)> = vec![
                ("TITLE=".into(), "".into()),
                ("NAME=".into(), prob.name.clone()),
                ("RUN=".into(), "".into()),
                ("INFILESUFFIX=".into(), "in".into()),
                ("ANSFILESUFFIX=".into(), "ans".into()),
                ("PLUG=".into(), format!("{}_e", prob.name)),
                (
                    "TYPE=".into(),
                    match prob.problem_type {
                        ProblemType::Program => "SOURCE".into(),
                        ProblemType::Output => {
                            warnings.push(format!(
                                "题目 {} 是提交答案型，Arbiter 可能不支持。",
                                prob.name
                            ));
                            "SOURCE".into()
                        }
                        ProblemType::Interactive => {
                            warnings
                                .push(format!("题目 {} 是交互型，Arbiter 可能不支持。", prob.name));
                            "SOURCE".into()
                        }
                    },
                ),
                ("LIMIT=".into(), prob.time_limit.as_secs_f64().to_string()),
                (
                    "MEMLIMITS=".into(),
                    (prob.memory_limit.as_u64() / 1024 / 1024).to_string(),
                ),
                ("SAMPLES=".into(), prob.samples.len().to_string()),
                ("CCL=c@gcc".into(), format!(" -o %o %i {}", c_args)),
                ("CCL=cpp@g++".into(), format!(" -o %o %i {}", cpp_args)),
                ("CCL=pas@fpc".into(), format!(" %i {}", pas_args)),
            ];

            // 复制数据文件（main/data 与 evaldata 各一份），写 MARK
            for (idx, case) in prob.data.iter().enumerate() {
                let idx = idx + 1;
                let in_name = format!("{}{}.in", prob.name, idx);
                let ans_name = format!("{}{}.ans", prob.name, idx);

                let input = doc.assets.load(prob.idx, &case.input).await?;
                let output = doc.assets.load(prob.idx, &case.output).await?;
                let eval_input = doc.assets.load(prob.idx, &case.input).await?;
                let eval_output = doc.assets.load(prob.idx, &case.output).await?;

                files.push(OutputFile::File {
                    path: PathBuf::from(format!("arbiter/main/data/{}", in_name)),
                    bytes: input,
                });
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("arbiter/main/data/{}", ans_name)),
                    bytes: output,
                });
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("arbiter/main/evaldata/{}", in_name)),
                    bytes: eval_input,
                });
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("arbiter/main/evaldata/{}", ans_name)),
                    bytes: eval_output,
                });

                let mark = if prob.subtasks.len() > 1 {
                    let subtask_score = prob
                        .subtasks
                        .get(&case.subtask)
                        .map(|st| st.max_score)
                        .unwrap_or(case.score);
                    let count_in_subtask = prob
                        .subtasks
                        .get(&case.subtask)
                        .map(|st| st.items.len())
                        .unwrap_or(1);
                    if count_in_subtask > 1 {
                        warnings.push(format!(
                            "题目 {} Subtask #{} 含多个测试点，Arbiter 不支持打包评测，将均分。",
                            prob.name, case.subtask
                        ));
                    }
                    subtask_score / count_in_subtask as u32
                } else {
                    score_per_case
                };

                probinfo.push((format!("MARK={}@", idx), mark.to_string()));
            }

            // Checker / filter
            if let Some(stream) = self.build_filter(doc, prob, &mut warnings).await? {
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("arbiter/main/filter/{}_e", prob.name)),
                    bytes: stream,
                });
            }

            files.push(OutputFile::File {
                path: PathBuf::from(format!("arbiter/main/task{}_{}.info", daynum, probnum)),
                bytes: Box::new(std::io::Cursor::new(build_info(&probinfo).into_bytes())),
            });
        }

        // setup.cfg
        let cfg: Vec<(String, String)> = vec![
            ("NAME=".into(), doc.config.contest_name.clone()),
            ("DAYNUM=".into(), daynum.to_string()),
            ("ENV=".into(), "env.info".into()),
            ("PLAYER=".into(), "player.info".into()),
            ("TEAM=".into(), "team.info".into()),
            ("MISC=".into(), "misc.info".into()),
        ];
        files.push(OutputFile::File {
            path: PathBuf::from("arbiter/main/setup.cfg"),
            bytes: Box::new(std::io::Cursor::new(build_info(&cfg).into_bytes())),
        });

        // 空的 team.info
        files.push(OutputFile::File {
            path: PathBuf::from("arbiter/main/team.info"),
            bytes: Box::new(std::io::Cursor::new(Vec::new())),
        });

        // 复制样例到 down/{day}/{name}/，含附加文件
        for prob in &doc.problems {
            info!("处理题目样例：{}", prob.name);
            let prob_down_dir = format!("arbiter/down/{}/{}", doc.config.day_name, prob.name);

            for (idx, sample) in prob.samples.iter().enumerate() {
                let idx = idx + 1;
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("{}/{}{}.in", prob_down_dir, prob.name, idx)),
                    bytes: doc.assets.load(prob.idx, &sample.input).await?,
                });
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("{}/{}{}.ans", prob_down_dir, prob.name, idx)),
                    bytes: doc.assets.load(prob.idx, &sample.output).await?,
                });
            }

            // 拷贝 down/ 目录下不属于 sample 的附加文件（保留文件树）
            for file in &prob.extra_down {
                let rel = file.path.strip_prefix("down/").unwrap_or(&file.path);
                info!("发现附加文件：{}", rel.display());
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("{}/{}", prob_down_dir, rel.display())),
                    bytes: doc.assets.load(prob.idx, &file.path).await?,
                });
            }
        }

        Ok((files, warnings))
    }
}
