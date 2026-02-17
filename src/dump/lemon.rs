use serde_json::{Map, Value, json};
use std::process::Command;

use crate::prelude::*;

pub fn main(day: &ContestDayConfig) -> Result<()> {
    let output_dir = day.path.join("dump/lemon");

    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)?;
    }
    fs::create_dir(&output_dir)?;
    fs::create_dir(&output_dir.join("data"))?;

    let mut prob_jsons: Vec<Value> = Vec::new();

    for (_, prob) in &day.subconfig {
        fs::create_dir(&output_dir.join("data").join(&prob.name))?;
        let mut cases: Vec<Value> = Vec::new();

        // 拷贝数据
        for case in &prob.data {
            fs::copy(
                prob.path.join("data").join(&case.input),
                &output_dir
                    .join("data")
                    .join(&prob.name)
                    .join(prob.name.clone() + &case.id.to_string() + ".in"),
            )?;
            fs::copy(
                prob.path.join("data").join(&case.output),
                &output_dir
                    .join("data")
                    .join(&prob.name)
                    .join(prob.name.clone() + &case.id.to_string() + ".ans"),
            )?;
        }

        // 处理配置文件
        for (_, task) in &prob.subtasks {
            match task.policy {
                ScorePolicy::Sum => {
                    for case in &task.items {
                        cases.push(Value::Object(
                            json!({
                                "fullScore": case.score,
                                "timeLimit": (prob.time_limit*1000.0) as u32,
                                "memoryLimit": prob.memory_limit.as_mib() as u32,
                                "inputFiles": json!([prob.name.clone() + "/" + &prob.name.clone() + &case.id.to_string() + ".in"]),
                                "outputFiles": json!([prob.name.clone() + "/" + &prob.name.clone() + &case.id.to_string() + ".ans"]),
                            })
                            .as_object()
                            .unwrap()
                            .to_owned(),
                        ));
                    }
                }
                ScorePolicy::Min => {
                    let mut inputs: Vec<Value> = Vec::new();
                    let mut outputs: Vec<Value> = Vec::new();
                    for case in &task.items {
                        inputs.push(Value::String(
                            prob.name.clone()
                                + "/"
                                + &prob.name.clone()
                                + &case.id.to_string()
                                + ".in",
                        ));
                        outputs.push(Value::String(
                            prob.name.clone()
                                + "/"
                                + &prob.name.clone()
                                + &case.id.to_string()
                                + ".ans",
                        ));
                    }
                    cases.push(Value::Object(
                        json!({
                            "fullScore": task.max_score,
                            "timeLimit": (prob.time_limit*1000.0) as u32,
                            "memoryLimit": prob.memory_limit.as_mib() as u32,
                            "inputFiles": inputs,
                            "outputFiles": outputs
                        })
                        .as_object()
                        .unwrap()
                        .to_owned(),
                    ));
                }
                ScorePolicy::Max => bail!("lemon 不支持 max 评分方法"),
            }
        }
        // debug!("{cases:#?}");

        // SPJ
        if prob.use_chk.unwrap_or(false) {
            info!("尝试编译 SPJ");

            let chk_path = prob.path.join("data").join("chk").join("chk.cpp");
            if !chk_path.exists() {
                bail!("chk 文件不存在");
            }
            let compile_status = Command::new("g++")
                .arg("-o")
                .arg(
                    &output_dir
                        .join("data")
                        .join(&prob.name)
                        .join("chk")
                        .with_extension(std::env::consts::EXE_EXTENSION),
                )
                .arg(&chk_path)
                .arg("-O2")
                .arg("-std=c++23")
                .status()?;

            if !compile_status.success() {
                bail!("SPJ 编译错误");
            }
        }

        // 组装这道题的 JSON
        let mut compilers: Map<String, Value> = Map::new();

        for (lang, _) in &day.compile {
            compilers.insert(
                match lang.as_str() {
                    "cpp" => "g++",
                    "c" => "gcc",
                    "pas" => "fpc",
                    "py" => "python",
                    "java" => "javac",
                    other => bail!("不支持的语言: {other}"),
                }
                .to_string(),
                Value::String("default".to_string()),
            );
        }

        let prob_json = json!({
            "answerFileExtension": "out",
            "comparisonMode": if prob.use_chk.unwrap_or(false) {4} else {1},
            "specialJudge": PathBuf::from(prob.name.clone())
                            .join("chk")
                            .with_extension(std::env::consts::EXE_EXTENSION),
            "diffArguments": "--ignore-space-change --text --brief",
            "inputFileName": prob.name.clone() + ".in",
            "outputFileName": prob.name.clone() + ".out",
            "problemTitle": prob.title,
            "taskType": match prob.problem_type{
                ProblemType::Program => 0,
                ProblemType::Output => 1,
                ProblemType::Interactive => bail!("lemon 不支持交互题"),
            },
            "compilerConfiguration": compilers,
            "testCases": cases
        });

        prob_jsons.push(prob_json);
    }

    // 组装全局 JSON

    let day_cdf = json!({
        "contestTitle": day.name,
        "contestants": Value::Array(Vec::new()),
        "tasks": prob_jsons
    });

    let cdf_file = output_dir.join(day.name.clone()).with_extension("cdf");

    fs::write(cdf_file, serde_json::to_string_pretty(&day_cdf)?)?;

    warn!("受 Lemon 限制，您需要手动调整编译选项。");
    warn!("目前设置是默认 (default)，如需要请自行修改。");

    Ok(())
}
