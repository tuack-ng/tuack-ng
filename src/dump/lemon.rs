use serde_json::{Map, Value, json};
use std::process::Command;

use crate::prelude::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LemonCase {
    full_score: u32,
    time_limit: u32,
    memory_limit: u32,
    input_files: Vec<String>,
    output_files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LemonProblem {
    answer_file_extension: String,
    comparison_mode: u32,
    special_judge: PathBuf,
    diff_arguments: String,
    input_file_name: String,
    output_file_name: String,
    problem_title: String,
    task_type: u32,
    compiler_configuration: Map<String, Value>,
    test_cases: Vec<LemonCase>,
}

const COMPILER_MAP: &[(&str, &str)] = &[
    ("cpp", "g++"),
    ("c", "gcc"),
    ("pas", "fpc"),
    ("py", "python"),
    ("java", "javac"),
];

fn compiler_for_lang(lang: &str) -> Result<&'static str> {
    COMPILER_MAP
        .iter()
        .find(|(k, _)| *k == lang)
        .map(|(_, v)| *v)
        .ok_or_else(|| anyhow!("不支持的语言：{lang}"))
}

fn case_rel_path(prob_name: &str, case_id: u32, ext: &str) -> String {
    format!("{prob_name}/{prob_name}{case_id}.{ext}")
}

fn copy_case_file(
    prob: &ProblemConfig,
    output_dir: &Path,
    src: &str,
    prob_name: &str,
    case_id: u32,
    ext: &str,
) -> Result<()> {
    fs::copy(
        prob.path.join("data").join(src),
        output_dir
            .join("data")
            .join(prob_name)
            .join(format!("{prob_name}{case_id}.{ext}")),
    )?;
    Ok(())
}

pub fn main(day: &ContestDayConfig) -> Result<()> {
    let output_dir = day.path.join("dump/lemon");

    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)?;
    }
    fs::create_dir(&output_dir)?;
    fs::create_dir(output_dir.join("data"))?;

    let mut prob_jsons: Vec<Value> = Vec::new();

    for (_, prob) in &day.subconfig {
        fs::create_dir(output_dir.join("data").join(&prob.name))?;

        for case in &prob.data {
            copy_case_file(prob, &output_dir, &case.input, &prob.name, case.id, "in")?;
            copy_case_file(prob, &output_dir, &case.output, &prob.name, case.id, "ans")?;
        }

        let mut cases: Vec<LemonCase> = Vec::new();
        let time_limit = (prob.time_limit * 1000.0) as u32;
        let memory_limit = prob.memory_limit.as_mib() as u32;

        for task in prob.subtasks.values() {
            let mut input_files: Vec<String> = Vec::new();
            let mut output_files: Vec<String> = Vec::new();

            for &idx in &task.items {
                let case = &prob.data[idx];
                input_files.push(case_rel_path(&prob.name, case.id, "in"));
                output_files.push(case_rel_path(&prob.name, case.id, "ans"));
            }

            match task.policy {
                ScorePolicy::Sum => {
                    for (i, &idx) in task.items.iter().enumerate() {
                        let case = &prob.data[idx];
                        cases.push(LemonCase {
                            full_score: case.score,
                            time_limit,
                            memory_limit,
                            input_files: vec![input_files[i].clone()],
                            output_files: vec![output_files[i].clone()],
                        });
                    }
                }
                ScorePolicy::Min => {
                    cases.push(LemonCase {
                        full_score: task.max_score,
                        time_limit,
                        memory_limit,
                        input_files,
                        output_files,
                    });
                }
                ScorePolicy::Max => bail!("lemon 不支持 max 评分方法"),
            }
        }

        if let Some(checker) = &prob.checker {
            info!("尝试编译 SPJ");

            let chk_path = prob.path.join(&checker.data.source);
            if !chk_path.exists() {
                bail!("checker 文件不存在：{}", chk_path.display());
            }
            let chk_out = output_dir
                .join("data")
                .join(&prob.name)
                .join("chk")
                .with_extension(std::env::consts::EXE_EXTENSION);
            let compile_status = Command::new("g++")
                .arg("-o")
                .arg(&chk_out)
                .arg(&chk_path)
                .arg("-O2")
                .arg("-std=c++23")
                .status()?;

            if !compile_status.success() {
                bail!("SPJ 编译错误");
            }
        }

        let mut compilers: Map<String, Value> = Map::new();
        for lang in day.compile.keys() {
            compilers.insert(
                compiler_for_lang(lang)?.to_string(),
                Value::String("default".to_string()),
            );
        }

        let prob_name = &prob.name;
        let task_type = match prob.problem_type {
            ProblemType::Program => 0,
            ProblemType::Output => 1,
            ProblemType::Interactive => bail!("lemon 不支持交互题"),
        };

        let prob_json = LemonProblem {
            answer_file_extension: "out".to_string(),
            comparison_mode: if prob.checker.is_some() { 4 } else { 1 },
            special_judge: PathBuf::from(prob_name)
                .join("chk")
                .with_extension(std::env::consts::EXE_EXTENSION),
            diff_arguments: "--ignore-space-change --text --brief".to_string(),
            input_file_name: format!("{prob_name}.in"),
            output_file_name: format!("{prob_name}.out"),
            problem_title: prob.title.clone(),
            task_type,
            compiler_configuration: compilers,
            test_cases: cases,
        };

        prob_jsons.push(serde_json::to_value(&prob_json)?);
    }

    let day_cdf = json!({
        "contestTitle": day.name,
        "contestants": Value::Array(Vec::new()),
        "tasks": prob_jsons
    });

    let cdf_file = output_dir.join(day.name.clone()).with_extension("cdf");
    fs::write(cdf_file, serde_json::to_string_pretty(&day_cdf)?)?;

    msg_warn!("受 Lemon 限制，您需要手动调整编译选项。");
    msg_warn!("目前设置是默认 (default)，如需要请自行修改。");

    Ok(())
}
