use log::{error, warn};
use serde_json;
use std::fs;

use crate::config::{
    ContestConfig, ContestDayConfig, DataJson, DateInfo, Problem, SupportLanguage, TemplateManifest,
};

use crate::context;

pub fn generate_data_json(
    contest_config: &ContestConfig,
    day_config: &ContestDayConfig,
) -> Result<DataJson, Box<dyn std::error::Error>> {
    // 构建问题列表
    let mut problems = Vec::new();

    for problem_config in &day_config.subconfig {
        let mut submit_filenames = Vec::new();

        // 遍历 day_config.compile 中的语言配置来生成对应的提交文件名
        for lang_key in day_config.compile.keys() {
            submit_filenames.push(format!("{}.{}", problem_config.name, lang_key));
        }

        let point_equal = if problem_config.data.is_empty() {
            // 如果没有测试数据，默认为"是"
            "是".to_string()
        } else {
            // 获取第一个测试点的分数
            let first_score = problem_config.data[0].score;
            // 检查所有测试点的分数是否都等于第一个测试点的分数
            let all_equal = problem_config
                .data
                .iter()
                .all(|data_item| data_item.score == first_score);

            if all_equal {
                "是".to_string()
            } else {
                "否".to_string()
            }
        };

        let problem = Problem {
            name: problem_config.name.clone(),
            title: problem_config.title.clone(),
            dir: problem_config.name.clone(), // 假设目录名就是问题名
            exec: problem_config.name.clone(), // 默认值，你可能需要从配置文件读取
            input: problem_config.name.clone() + ".in",
            output: problem_config.name.clone() + ".ans", // 改为使用 .ans 后缀
            problem_type: match problem_config.problem_type.as_str() {
                "program" => "传统型",
                "output" => "提交答案型",
                "interactive" => "交互型",
                _ => {
                    warn!(
                        "未知的题目类型 {} , 使用默认值: 传统型",
                        problem_config.problem_type
                    );
                    "传统型"
                }
            }
            .to_string(),
            time_limit: format!("{:.1} 秒", problem_config.time_limit),
            memory_limit: problem_config.memory_limit.clone(),
            testcase: problem_config.data.len().to_string(),
            point_equal,
            submit_filename: submit_filenames,
        };
        problems.push(problem);
    }

    // 构建支持的语言列表
    let context = crate::context::get_context();
    let mut support_languages = Vec::new();

    for (lang_key, compile_options) in &day_config.compile {
        // 从context中查找对应的语言配置来获取语言名称
        let language_name = if let Some(lang_config) = context.languages.get(lang_key) {
            lang_config.language.clone()
        } else {
            // 如果context中没有对应的语言配置，使用键名作为语言名称
            error!("在语言配置中未找到 {}", lang_key);
            return Err(format!("在语言配置中未找到 {}", lang_key).into());
        };

        let language = SupportLanguage {
            name: language_name,
            compile_options: compile_options.clone(),
        };
        support_languages.push(language);
    }

    // 创建日期信息
    let date = DateInfo {
        start: day_config.start_time,
        end: day_config.end_time,
    };

    // 读取模板目录中的清单文件以获取默认值
    let manifest_path = context::get_context().assets_dirs.iter().find_map(|dir| {
        let manifest_file = dir.join("templates").join("noi").join("manifest.json");
        if manifest_file.exists() {
            Some(manifest_file)
        } else {
            None
        }
    });

    let manifest = if let Some(path) = manifest_path {
        let manifest_content = fs::read_to_string(&path)?;
        serde_json::from_str::<TemplateManifest>(&manifest_content)?
    } else {
        error!("找不到清单文件");
        return Err("致命错误: 找不到清单文件".into());
    };

    // 从ContestConfig和ContestDayConfig中获取覆盖值
    let use_pretest = day_config
        .use_pretest
        .or(contest_config.use_pretest)
        .unwrap_or(manifest.use_pretest);
    let noi_style = day_config
        .noi_style
        .or(contest_config.noi_style)
        .unwrap_or(manifest.noi_style);
    let file_io = day_config
        .file_io
        .or(contest_config.file_io)
        .unwrap_or(manifest.file_io);

    Ok(DataJson {
        title: contest_config.title.clone(),
        subtitle: contest_config.short_title.clone(),
        dayname: day_config.title.clone(),
        date,
        use_pretest,
        noi_style,
        file_io,
        support_languages,
        problems,
    })
}
