//! CCR-Plus 导出器。
//!
//! 输出为 CCR-Plus 的竞赛目录：
//!
//! ```text
//! ccr-plus/
//!   .ccr                            竞赛信息（题目顺序）
//!   data/<题目名>/
//!     .prb                           题目配置
//!     1.in 1.ans ...                 数据文件
//!     <校验器>                       自定义 SPJ 可执行文件（由本导出器编译生成）
//!   src/                             选手源文件目录（空）
//!   result/                          结果目录（空）
//! ```
//!
//! CCR-Plus 采用文件 IO 约定（`<题目名>.in` / `<题目名>.out`）。
//! 传统题按该约定导出；提交答案型输出 `sub` 提交文件。
//! 自定义 SPJ 与 lemon/arbiter 一致地编译生成；依赖文件随源码一起拷贝。

use std::process::Command;

use quick_xml::se::to_string;
use strfmt::strfmt;

use crate::prelude::*;
use tuack_lib::dump::{DumpProblem, Dumper, ScorePolicy};
use tuack_lib::ren::ProblemType;
use tuack_lib::utils::output::OutputFile;

/// CCR-Plus 配置版本号（写入 .prb / .ccr 的 `version` 属性）
const CCR_VERSION: &str = "1.1.0";

/// 默认编译时限（秒），沿用 CCR-Plus 的内置编译器默认值
const COMPILE_TIME: u32 = 10;

/// 默认校验器时限（秒）
const CHECKER_TIME: u32 = 10;

/// 默认代码长度限制 (KB)，沿用 CCR-Plus 默认值
const DEFAULT_CODE_LEN: u32 = 100;

/// 推荐编译语言集合（无编译配置时的兜底）
const DEFAULT_LANGS: &[&str] = &["c", "cpp", "pas"];

/// 语言 -> (编译命令模板，源文件模板)
/// 模板中 `{source}` 为源文件名（不含扩展名），`{exe}` 为输出文件名（不含扩展名）
/// CCR-Plus 只支持编译型语言（c/cpp/pas），其余语言会告警并略过。
fn compiler_cmd(lang: &str) -> Option<(&'static str, &'static str)> {
    match lang {
        "cpp" => Some(("g++ -o {exe} {source}.cpp -lm -static", "{source}.cpp")),
        "c" => Some(("gcc -o {exe} {source}.c -lm -static", "{source}.c")),
        "pas" => Some(("fpc {source}.pas", "{source}.pas")),
        _ => None,
    }
}

#[derive(Serialize)]
#[serde(rename = "problem")]
struct PrbProblem {
    #[serde(rename = "@type")]
    ty: String,
    #[serde(rename = "@maker")]
    maker: String,
    #[serde(rename = "@version")]
    version: String,
    #[serde(rename = "source")]
    source: PrbSource,
    #[serde(rename = "task")]
    task: PrbTask,
}

#[derive(Serialize)]
struct PrbSource {
    #[serde(rename = "@dir")]
    dir: String,
    #[serde(rename = "@file", skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(rename = "@code", skip_serializing_if = "Option::is_none")]
    code: Option<u32>,
    #[serde(rename = "language", skip_serializing_if = "Vec::is_empty")]
    languages: Vec<PrbLanguage>,
}

#[derive(Serialize)]
struct PrbLanguage {
    #[serde(rename = "@cmd")]
    cmd: String,
    #[serde(rename = "@file")]
    file: String,
    #[serde(rename = "@time")]
    time: u32,
}

#[derive(Serialize)]
struct PrbTask {
    #[serde(rename = "@input", skip_serializing_if = "Option::is_none")]
    input: Option<String>,
    #[serde(rename = "@output", skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    #[serde(rename = "@checker")]
    checker: String,
    #[serde(rename = "@time")]
    time: u32,
    #[serde(rename = "subtask", skip_serializing_if = "Vec::is_empty")]
    subtasks: Vec<PrbSubtask>,
}

#[derive(Serialize)]
struct PrbSubtask {
    #[serde(rename = "@score")]
    score: u32,
    #[serde(rename = "point", skip_serializing_if = "Vec::is_empty")]
    points: Vec<PrbPoint>,
}

#[derive(Serialize)]
struct PrbPoint {
    #[serde(rename = "@in")]
    input: String,
    #[serde(rename = "@out")]
    output: String,
    #[serde(rename = "@sub", skip_serializing_if = "Option::is_none")]
    submit: Option<String>,
    #[serde(rename = "@time", skip_serializing_if = "Option::is_none")]
    time: Option<f64>,
    #[serde(rename = "@mem", skip_serializing_if = "Option::is_none")]
    mem: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename = "contest")]
struct CcrContest {
    #[serde(rename = "@maker")]
    maker: String,
    #[serde(rename = "@version")]
    version: String,
    #[serde(rename = "order")]
    order: CcrOrder,
}

#[derive(Serialize)]
struct CcrOrder {
    #[serde(rename = "problem")]
    problems: Vec<String>,
}

fn xml_decl(content: String) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{content}")
}

/// 取资源逻辑路径的文件名（如 `data/1.in` -> `1.in`）
fn rel_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// 把 CCR-Plus 命令模板（`{source}`/`{exe}`）替换为题目的具体名称。
/// 仅支持编译型语言；不支持的语言略过并告警。
fn build_languages(
    name: &str,
    compile: &[(String, String)],
    warnings: &mut Vec<String>,
) -> Result<Vec<PrbLanguage>> {
    let langs: Vec<String> = if compile.is_empty() {
        DEFAULT_LANGS.iter().map(|s| s.to_string()).collect()
    } else {
        compile.iter().map(|(k, _)| k.clone()).collect()
    };

    let vars = HashMap::from([
        ("source".to_string(), name.to_string()),
        ("exe".to_string(), name.to_string()),
    ]);

    let mut out = Vec::new();
    for lang in langs {
        if let Some((base, file_pat)) = compiler_cmd(&lang) {
            let opts = compile
                .iter()
                .find(|(k, _)| *k == lang)
                .map(|(_, v)| v.as_str())
                .unwrap_or("");
            let base = strfmt(base, &vars)?;
            let file = strfmt(file_pat, &vars)?;
            let cmd = if opts.trim().is_empty() {
                base
            } else {
                format!("{base} {opts}")
            };
            out.push(PrbLanguage {
                cmd,
                file,
                time: COMPILE_TIME,
            });
        } else {
            warnings.push(format!("语言 {lang} 不受 CCR-Plus 支持，已略过。"));
        }
    }
    Ok(out)
}

/// 生成单个测试点
fn make_point(prob: &DumpProblem, case: &tuack_lib::dump::DumpCase) -> PrbPoint {
    let is_output = prob.problem_type == ProblemType::Output;
    let output = rel_name(&case.output);
    PrbPoint {
        input: rel_name(&case.input),
        output: rel_name(&case.output),
        submit: if is_output { Some(output) } else { None },
        time: if is_output {
            None
        } else {
            Some(prob.time_limit.as_secs_f64())
        },
        mem: if is_output {
            None
        } else {
            Some(prob.memory_limit.as_mib())
        },
    }
}

/// 依评分策略把测试点整理成 CCR-Plus 的 subtask 列表。
/// `Max` 无法表达，报错。
fn build_subtasks(prob: &DumpProblem) -> Result<Vec<PrbSubtask>> {
    let mut subtasks = Vec::new();

    // 未配置 subtask 时，把每个测试点视为独立的求和子任务
    if prob.subtasks.is_empty() {
        for case in &prob.data {
            subtasks.push(PrbSubtask {
                score: case.score,
                points: vec![make_point(prob, case)],
            });
        }
        return Ok(subtasks);
    }

    for sub in prob.subtasks.values() {
        match sub.policy {
            ScorePolicy::Sum => {
                for &idx in &sub.items {
                    let case = &prob.data[idx];
                    subtasks.push(PrbSubtask {
                        score: case.score,
                        points: vec![make_point(prob, case)],
                    });
                }
            }
            ScorePolicy::Min => {
                let points = sub
                    .items
                    .iter()
                    .map(|&idx| make_point(prob, &prob.data[idx]))
                    .collect();
                subtasks.push(PrbSubtask {
                    score: sub.max_score,
                    points,
                });
            }
            ScorePolicy::Max => bail!("题目 {} 使用 max 评分方法，CCR-Plus 不支持。", prob.name),
        }
    }

    Ok(subtasks)
}

fn build_prb(
    prob: &DumpProblem,
    checker: &str,
    compile: &[(String, String)],
    warnings: &mut Vec<String>,
) -> Result<String> {
    let ty = match prob.problem_type {
        ProblemType::Program => "TRA_0_4",
        ProblemType::Output => "ANS_0_4",
        ProblemType::Interactive => bail!("ccr-plus 不支持交互题"),
    };

    let subtasks = build_subtasks(prob)?;

    let source = match prob.problem_type {
        ProblemType::Program => PrbSource {
            dir: prob.name.clone(),
            file: Some(prob.name.clone()),
            code: Some(DEFAULT_CODE_LEN),
            languages: build_languages(&prob.name, compile, warnings)?,
        },
        ProblemType::Output => PrbSource {
            dir: prob.name.clone(),
            file: None,
            code: None,
            languages: Vec::new(),
        },
        ProblemType::Interactive => unreachable!(),
    };

    let task = PrbTask {
        input: if prob.problem_type == ProblemType::Program {
            Some(format!("{}.in", prob.name))
        } else {
            None
        },
        output: if prob.problem_type == ProblemType::Program {
            Some(format!("{}.out", prob.name))
        } else {
            None
        },
        checker: checker.to_string(),
        time: CHECKER_TIME,
        subtasks,
    };

    let prb = PrbProblem {
        ty: ty.to_string(),
        maker: "ccr-plus".to_string(),
        version: CCR_VERSION.to_string(),
        source,
        task,
    };

    Ok(xml_decl(to_string(&prb)?))
}

fn build_ccr(order: &[String]) -> String {
    let contest = CcrContest {
        maker: "ccr-plus".to_string(),
        version: CCR_VERSION.to_string(),
        order: CcrOrder {
            problems: order.to_vec(),
        },
    };
    xml_decl(to_string(&contest).expect("ccr plus contest 序列化失败"))
}

pub struct CcrPlusDumper {
    tmp_dir: PathBuf,
}

impl CcrPlusDumper {
    pub fn new(tmp_dir: PathBuf) -> Self {
        Self { tmp_dir }
    }

    /// 编译自定义 SPJ：源码与依赖经 assets 读取写入 tmp，再 g++ 编译。
    /// 返回可执行文件名（含平台后缀）。
    async fn compile_checker(
        &self,
        doc: &tuack_lib::dump::DumpDocument,
        prob: &DumpProblem,
    ) -> Result<String> {
        let checker = prob.checker.as_ref().context("无校验器配置")?;
        let stem = checker
            .source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "chk".to_string());
        let exe_name = format!("{stem}{}", std::env::consts::EXE_SUFFIX);

        info!("尝试编译 SPJ：{}", checker.source.display());

        let src_tmp = self.tmp_dir.join("ccr-chk-src.cpp");
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

        let chk_out = self.tmp_dir.join(&exe_name);
        let status = Command::new("g++")
            .arg("-o")
            .arg(&chk_out)
            .arg(&src_tmp)
            .arg("-O2")
            .arg("-std=c++17")
            .status()
            .context("执行 g++ 失败")?;
        if !status.success() {
            bail!("SPJ 编译错误：{}", checker.source.display());
        }

        Ok(exe_name)
    }
}

#[async_trait]
impl Dumper for CcrPlusDumper {
    async fn dump(
        &self,
        doc: &tuack_lib::dump::DumpDocument,
    ) -> Result<(Vec<OutputFile>, Vec<String>)> {
        let mut files = Vec::new();
        let mut warnings = Vec::new();

        for prob in &doc.problems {
            let pdir = format!("ccr-plus/data/{}", prob.name);

            // 复制数据文件（输入/输出）
            for case in &prob.data {
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("{}/{}", pdir, rel_name(&case.input))),
                    bytes: doc.assets.load(prob.idx, &case.input).await?,
                });
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("{}/{}", pdir, rel_name(&case.output))),
                    bytes: doc.assets.load(prob.idx, &case.output).await?,
                });
            }

            // 校验器：有自定义 SPJ 则编译生成；否则用内置全文比较
            let checker = if let Some(checker) = &prob.checker {
                let exe_name = self.compile_checker(doc, prob).await?;
                let stem = checker
                    .source
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "chk".to_string());
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("{}/{}", pdir, exe_name)),
                    bytes: Box::new(tokio::fs::File::open(self.tmp_dir.join(&exe_name)).await?),
                });
                // `.prb` 的 `@checker` 写入不带扩展名的基名：CCR-Plus 在 Windows 上
                // 会自行补充 `.exe`（AddFileExtension），故二进制文件名与属性名需分开。
                stem
            } else {
                "fulltext".to_string()
            };

            // 写 .prb
            let prb = build_prb(prob, &checker, &doc.config.compile, &mut warnings)?;
            files.push(OutputFile::File {
                path: PathBuf::from(format!("{}/.prb", pdir)),
                bytes: Box::new(std::io::Cursor::new(prb.into_bytes())),
            });
        }

        // 竞赛信息 .ccr（题目顺序）
        let order: Vec<String> = doc.problems.iter().map(|p| p.name.clone()).collect();
        files.push(OutputFile::File {
            path: PathBuf::from("ccr-plus/.ccr"),
            bytes: Box::new(std::io::Cursor::new(build_ccr(&order).into_bytes())),
        });

        // 空目录：选手源文件与结果
        files.push(OutputFile::Dir(PathBuf::from("ccr-plus/src")));
        files.push(OutputFile::Dir(PathBuf::from("ccr-plus/result")));

        warnings.push(
            "CCR-Plus 使用文件 IO（`<题目名>.in` / `<题目名>.out`），请确认题目及标程采用该接口。"
                .to_string(),
        );

        Ok((files, warnings))
    }
}
