use std::process::Command;

use crate::prelude::*;

/// 写入 arbiter 的 key=value 格式配置文件
fn write_info(path: &Path, info: &[(String, String)]) -> Result<()> {
    let mut content = String::new();
    for (key, val) in info {
        content.push_str(key);
        content.push_str(val);
        content.push('\n');
    }
    fs::write(path, content.as_bytes())?;
    Ok(())
}

/// arbiter_main 的内层逻辑：处理单个 day
fn arbiter_main_day(day: &ContestDayConfig, daynum: usize, main_dir: &Path) -> Result<()> {
    // 写 day{N}.info
    let dayinfo: Vec<(String, String)> = vec![
        ("NAME=".into(), format!("第{}场--机试", daynum)),
        ("PLAYERDIR=".into(), "".into()),
        ("CASEDIR=".into(), "".into()),
        ("BASESCORE=".into(), "0".into()),
        ("TASKNUM=".into(), day.subconfig.len().to_string()),
    ];
    write_info(&main_dir.join(format!("day{}.info", daynum)), &dayinfo)?;

    for (probnum, (_, prob)) in day.subconfig.iter().enumerate() {
        let probnum = probnum + 1;
        info!("处理题目: {}", prob.name);

        let score_per_case = if prob.data.is_empty() {
            0u32
        } else {
            100 / prob.data.len() as u32
        };

        if !prob.data.is_empty()
            && prob.subtasks.len() <= 1
            && score_per_case * prob.data.len() as u32 != 100
        {
            warn!(
                "题目 {} 的测试点数量不是 100 的约数，分数无法均分为整数。",
                prob.name
            );
        }

        let compile = &day.compile;
        let c_args = compile.get("c").cloned().unwrap_or_default();
        let cpp_args = compile.get("cpp").cloned().unwrap_or_default();
        let pas_args = compile.get("pas").cloned().unwrap_or_default();

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
                        warn!("题目 {} 是提交答案型，Arbiter 可能不支持。", prob.name);
                        "SOURCE".into()
                    }
                    ProblemType::Interactive => {
                        warn!("题目 {} 是交互型，Arbiter 可能不支持。", prob.name);
                        "SOURCE".into()
                    }
                },
            ),
            ("LIMIT=".into(), prob.time_limit.to_string()),
            (
                "MEMLIMITS=".into(),
                (prob.memory_limit.as_u64() / 1024 / 1024).to_string(),
            ),
            ("SAMPLES=".into(), prob.samples.len().to_string()),
            ("CCL=c@gcc".into(), format!(" -o %o %i {}", c_args)),
            ("CCL=cpp@g++".into(), format!(" -o %o %i {}", cpp_args)),
            ("CCL=pas@fpc".into(), format!(" %i {}", pas_args)),
        ];

        // 复制数据文件，写 MARK
        for (idx, case) in prob.data.iter().enumerate() {
            let idx = idx + 1;

            let src_in = prob.path.join("data").join(&case.input);
            let src_ans = prob.path.join("data").join(&case.output);
            let dst_in = main_dir
                .join("data")
                .join(format!("{}{}.in", prob.name, idx));
            let dst_ans = main_dir
                .join("data")
                .join(format!("{}{}.ans", prob.name, idx));

            fs::copy(&src_in, &dst_in).with_context(|| {
                format!("复制 {} -> {} 失败", src_in.display(), dst_in.display())
            })?;
            fs::copy(&src_ans, &dst_ans).with_context(|| {
                format!("复制 {} -> {} 失败", src_ans.display(), dst_ans.display())
            })?;

            // 分数：packed 模式（subtask min/max）下使用 subtask 分数均分，否则均分
            let mark = if prob.subtasks.len() > 1 {
                // 找到这个 case 所在的 subtask
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
                    warn!(
                        "题目 {} Subtask #{} 含多个测试点，Arbiter 不支持打包评测，将均分。",
                        prob.name, case.subtask
                    );
                }
                subtask_score / count_in_subtask as u32
            } else {
                score_per_case
            };

            probinfo.push((format!("MARK={}@", idx), mark.to_string()));
        }

        // Checker
        let chk_cpp = prob.path.join("data").join("chk").join("chk.cpp");
        let filter_path = main_dir.join("filter").join(format!("{}_e", prob.name));

        if prob.use_chk.unwrap_or(false) && chk_cpp.exists() {
            info!("发现 chk，尝试编译。");
            let status = Command::new("g++")
                .arg(&chk_cpp)
                .arg("-o")
                .arg(&filter_path)
                .arg("-O2")
                .arg("-std=c++17")
                .status()
                .context("执行 g++ 失败")?;

            if !status.success() {
                warn!("chk 编译失败，请手动处理: {}", chk_cpp.display());
            }
        } else {
            // 从 assets 中复制默认的 arbiter_e 可执行文件（如果有）
            let sample = get_context().assets_dirs.iter().find_map(|d| {
                let p = d.join("sample").join("arbiter_e.sample");
                p.exists().then_some(p)
            });

            match sample {
                Some(src) => {
                    fs::copy(&src, &filter_path).with_context(|| {
                        format!(
                            "复制默认 arbiter_e 失败: {} -> {}",
                            src.display(),
                            filter_path.display()
                        )
                    })?;
                }
                None => {
                    warn!(
                        "题目 {} 没有 chk，也没有找到默认的 arbiter_e.sample，filter 目录为空。",
                        prob.name
                    );
                }
            }
        }

        write_info(
            &main_dir.join(format!("task{}_{}.info", daynum, probnum)),
            &probinfo,
        )?;
    }

    Ok(())
}

/// arbiter_down：复制样例文件
fn arbiter_down_day(day: &ContestDayConfig, down_dir: &Path) -> Result<()> {
    for (_, prob) in &day.subconfig {
        info!("处理题目样例: {}", prob.name);

        let prob_down_dir = down_dir.join(&prob.name);
        if !prob_down_dir.exists() {
            fs::create_dir_all(&prob_down_dir)?;
        }

        for (idx, sample) in prob.samples.iter().enumerate() {
            let idx = idx + 1;

            let input_name = sample
                .input
                .get()
                .cloned()
                .unwrap_or(format!("{}.in", sample.id));
            let output_name = sample
                .output
                .get()
                .cloned()
                .unwrap_or(format!("{}.ans", sample.id));

            let src_in = prob.path.join("sample").join(&input_name);
            let src_ans = prob.path.join("sample").join(&output_name);

            if src_in.exists() {
                let dst = prob_down_dir.join(format!("{}{}.in", prob.name, idx));
                fs::copy(&src_in, &dst)
                    .with_context(|| format!("复制样例输入失败: {}", src_in.display()))?;
            } else {
                warn!("样例输入文件不存在: {}", src_in.display());
            }

            if src_ans.exists() {
                let dst = prob_down_dir.join(format!("{}{}.ans", prob.name, idx));
                fs::copy(&src_ans, &dst)
                    .with_context(|| format!("复制样例输出失败: {}", src_ans.display()))?;
            } else {
                warn!("样例输出文件不存在: {}", src_ans.display());
            }

            // 拷贝 down/ 目录下其他不在 samples 列表里的文件（与原 Python 版一致）
        }

        // 拷贝 prob.path/down/ 下不属于 sample 的附加文件
        let extra_down = prob.path.join("down");
        if extra_down.exists() {
            let sample_files: std::collections::HashSet<String> = prob
                .samples
                .iter()
                .flat_map(|s| {
                    let i = s.input.get().cloned().unwrap_or(format!("{}.in", s.id));
                    let o = s.output.get().cloned().unwrap_or(format!("{}.ans", s.id));
                    [i, o]
                })
                .collect();

            for entry in fs::read_dir(&extra_down)? {
                let entry = entry?;
                let fname = entry.file_name().to_string_lossy().to_string();
                if !sample_files.contains(&fname) {
                    info!("发现附加文件: {}", fname);
                    fs::copy(entry.path(), prob_down_dir.join(&fname))
                        .with_context(|| format!("复制附加文件 {} 失败", fname))?;
                }
            }
        }
    }

    Ok(())
}

pub fn main(contest: &ContestConfig, day: &ContestDayConfig, daynum: usize) -> Result<()> {
    let out_root = day.path.join("dump").join("arbiter");

    // --- 初始化目录结构 ---
    let main_dir = out_root.join("main");
    for sub in &["data", "final", "players", "result", "filter", "tmp"] {
        let p = main_dir.join(sub);
        if !p.exists() {
            fs::create_dir_all(&p)?;
        }
    }
    let players_day = main_dir.join("players").join(format!("day{}", daynum));
    if !players_day.exists() {
        fs::create_dir_all(&players_day)?;
    }
    let result_day = main_dir.join("result").join(format!("day{}", daynum));
    if !result_day.exists() {
        fs::create_dir_all(&result_day)?;
    }

    // --- arbiter_main ---
    arbiter_main_day(day, daynum, &main_dir)?;

    // setup.cfg（只在最后一个 day / 单 day 调用时写，由 dump.rs 控制；
    // 这里每次都写，多 day 时会被覆盖为最后一个，符合原 Python 行为）
    let cfg: Vec<(String, String)> = vec![
        ("NAME=".into(), contest.name.clone()),
        ("DAYNUM=".into(), daynum.to_string()),
        ("ENV=".into(), "env.info".into()),
        ("PLAYER=".into(), "player.info".into()),
        ("TEAM=".into(), "team.info".into()),
        ("MISC=".into(), "misc.info".into()),
    ];
    write_info(&main_dir.join("setup.cfg"), &cfg)?;

    // 空的 team.info
    write_info(&main_dir.join("team.info"), &[])?;

    // evaldata：拷贝 data 目录（arbiter 需要两份）
    let evaldata = main_dir.join("evaldata");
    if evaldata.exists() {
        fs::remove_dir_all(&evaldata)?;
    }
    crate::utils::filesystem::copy_dir_recursive(main_dir.join("data"), &evaldata)?;

    // --- arbiter_down ---
    let down_dir = out_root.join("down").join(&day.name);
    if !down_dir.exists() {
        fs::create_dir_all(&down_dir)?;
    }
    arbiter_down_day(day, &down_dir)?;

    info!("Arbiter 导出完成，输出目录: {}", out_root.display());

    Ok(())
}
