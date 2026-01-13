pub mod data_json;
pub mod template;
pub mod utils;

use crate::config::ProblemConfig;
use clap::Args;
use log::{debug, error, info, warn};
use markdown_ppp::parser::*;
use markdown_ppp::typst_printer::config::Config;
use markdown_ppp::typst_printer::render_typst;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use data_json::generate_data_json;
use template::render_template;
use utils::{copy_dir_recursive, process_image_urls, process_images_with_unique_ids};

use indicatif::ProgressBar;

use crate::context;
use crate::context::{CurrentLocation, get_context};

#[derive(Args, Debug)]
#[command(version)]
pub struct RenArgs {
    /// 渲染目标模板
    #[arg(required = true)]
    pub target: String,

    /// 保留临时目录用于调试
    #[arg(long)]
    pub keep_tmp: bool,
}

pub fn main(args: RenArgs) -> Result<(), Box<dyn std::error::Error>> {
    debug!(
        "当前目录: {}",
        Path::new(".").canonicalize()?.to_string_lossy()
    );

    let (config, current_location) = get_context().config.as_ref().ok_or("找不到配置文件")?;

    // 根据当前位置确定skip_level
    let skip_level = match current_location {
        CurrentLocation::Problem(_, _) => 2,
        CurrentLocation::Day(_) => 1,
        _ => 0,
    };

    // 获取目标天和问题的名称
    let (target_day_name, target_problem_name) = match current_location {
        CurrentLocation::Problem(day_name, problem_name) => (Some(day_name), Some(problem_name)),
        CurrentLocation::Day(day_name) => (Some(day_name), None),
        _ => (None, None),
    };

    let template_dir = context::get_context().assets_dirs.iter().find(|dir| {
        let subdir = dir.join("templates").join(&args.target);
        subdir.exists() && subdir.is_dir()
    });

    let template_dir = match template_dir {
        Some(dir) => {
            info!(
                "找到模板目录: {}",
                dir.join("templates").join(&args.target).to_string_lossy()
            );
            dir.join("templates").join(&args.target)
        }
        None => {
            error!("没有找到模板 {}", args.target);
            return Err(format!("致命错误: 没有找到模板 {}", args.target).into());
        }
    };

    debug!("检查Typst编译环境");
    let typst_check = Command::new("typst").arg("--version").output();

    match typst_check {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                debug!("Typst 版本: {}", version.trim());
            } else {
                error!("Typst 命令执行失败，请检查是否已安装");
                return Err("Typst 命令执行失败，请检查是否已安装".into());
            }
        }
        Err(_) => {
            return Err("未找到 typst 命令，请确保已安装并添加到PATH".into());
        }
    }

    let template_required_files = ["main.typ", "utils.typ"];
    for file in template_required_files {
        if !template_dir.join(file).exists() {
            error!("模板缺少必要文件: {}", file);
            return Err(format!("模板缺少必要文件: {}", file).into());
        }
        info!("文件存在: {}", file);
    }

    let statements_dir = config.path.join("statements/");

    let statements_dir = match current_location {
        CurrentLocation::Problem(day_name, problem_name) => {
            let problem_dir = Path::new(&config.path).join(day_name).join(problem_name);

            problem_dir.join("statement")
        }
        CurrentLocation::Day(day_name) => {
            let day_dir = Path::new(&config.path).join(day_name);

            day_dir.join("statement")
        }
        _ => statements_dir.clone(),
    };

    info!("{}", &statements_dir.to_string_lossy());
    if !statements_dir.exists() {
        info!("创建题面输出目录: {}", statements_dir.display());
        fs::create_dir(&statements_dir)?;
    }

    // 计算要渲染的总天数
    let total_days = if skip_level >= 1 {
        1
    } else {
        config.subconfig.len()
    };

    // 添加天级别进度条（仅当需要处理多天时显示）
    let day_pb = if skip_level >= 1 {
        // 如果跳过天级别，就不显示进度条
        get_context().multiprogress.add(ProgressBar::new(0))
    } else {
        get_context()
            .multiprogress
            .add(ProgressBar::new(total_days as u64))
    };

    if skip_level < 1 {
        day_pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("  [{bar:40.green/blue}] {msg}")
                .unwrap()
                .progress_chars("=> "),
        );
    }

    let mut day_count = 0;
    for day in &config.subconfig {
        // 如果设置了跳过天级别，并且这不是目标天，则跳过
        if skip_level >= 1
            && target_day_name.is_some()
            && day.name != target_day_name.as_ref().unwrap().to_string()
        {
            continue;
        }

        day_count += 1;
        if skip_level < 1 {
            day_pb.set_message(format!("处理第 {}/{} 天", day_count, total_days));
        }
        info!("处理天: {}", day.name);

        if !statements_dir.exists() {
            fs::create_dir(&statements_dir)?;
            info!("创建输出目录: {}", statements_dir.display());
        }

        let output_filename = format!("{}.pdf", day.name);

        // 创建临时目录
        let tmp_dir = statements_dir.join("tmp");
        if tmp_dir.exists() {
            info!("清理已存在的临时目录: {}", tmp_dir.display());
            fs::remove_dir_all(&tmp_dir)?;
        }
        fs::create_dir(&tmp_dir)?;
        info!("创建临时目录: {}", tmp_dir.display());

        info!("复制模板文件到临时目录");
        copy_dir_recursive(&template_dir, &tmp_dir)?;

        // 计算要渲染的总问题数
        let problems_to_render: Vec<&ProblemConfig> = if skip_level >= 2
            && target_problem_name.is_some()
        {
            // 只渲染特定问题
            match day
                .subconfig
                .iter()
                .find(|p| p.name == target_problem_name.as_ref().unwrap().to_string())
            {
                Some(problem) => {
                    info!("渲染指定问题: {}", problem.name);
                    vec![problem]
                }
                None => {
                    error!("未找到问题: {}", target_problem_name.as_ref().unwrap());
                    return Err(
                        format!("未找到问题: {}", target_problem_name.as_ref().unwrap()).into(),
                    );
                }
            }
        } else {
            // 渲染所有问题
            info!("渲染所有问题（共{}个）", day.subconfig.len());
            day.subconfig.iter().collect()
        };

        // 生成并写入 data.json
        let data_json = if skip_level >= 2 && target_problem_name.is_some() {
            // 创建一个只包含当前天和当前题目的简化配置
            let mut modified_day = day.clone();

            // 只保留当前题目
            modified_day.subconfig = problems_to_render.iter().map(|&p| p.clone()).collect();

            generate_data_json(config, &modified_day)?
        } else {
            generate_data_json(config, day)?
        };

        let data_json_str = serde_json::to_string_pretty(&data_json)?;
        fs::write(tmp_dir.join("data.json"), data_json_str)?;
        info!("生成 data.json");

        // 添加问题级别进度条
        let problem_pb = get_context()
            .multiprogress
            .add(ProgressBar::new(problems_to_render.len() as u64));

        problem_pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("  [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=> "),
        );

        for (idx, problem) in problems_to_render.iter().enumerate() {
            // 题目级别时固定索引为0，其他情况使用原始索引
            let typst_index = if skip_level >= 2 {
                0
            } else {
                day.subconfig
                    .iter()
                    .position(|p| p.name == problem.name)
                    .unwrap_or(idx)
            };

            problem_pb.set_message(format!("处理问题: {}", problem.name));

            info!("处理问题: {}", problem.name);

            // 题面文件路径
            let problem_dir = &problem.path;
            let statement_path = problem_dir.join("statement.md");

            if !statement_path.exists() {
                error!("未找到题面文件: {}", statement_path.display());
                problem_pb.finish_with_message("遇到错误，停止处理");
                return Err(format!("未找到题面文件: {}", statement_path.display()).into());
            }

            // 解析题面顺便展开模板
            let content = match render_template(
                &fs::read_to_string(&statement_path)?,
                problem,
                day,
                config,
                problem.path.clone(),
            ) {
                Ok(content) => content,
                Err(e) => {
                    error!(
                        "读取题面文件/展开模板 {} 失败: {:?}",
                        statement_path.display(),
                        e
                    );
                    problem_pb.finish_with_message("遇到错误，停止处理");
                    return Err("解析题面文件失败".into());
                }
            };

            let state = MarkdownParserState::new();
            let mut ast = match parse_markdown(state, &content) {
                Ok(ast) => ast,
                Err(e) => {
                    error!("解析题面文件 {} 失败: {:?}", statement_path.display(), e);
                    problem_pb.finish_with_message("遇到错误，停止处理");
                    return Err("解析题面文件失败".into());
                }
            };

            let img_src_dir = problem_dir.join("img");

            process_image_urls(&img_src_dir, &mut ast);

            info!("生成Typst: {}", problem.name);
            let typst_output = render_typst(&ast, Config::default().with_width(1000000));
            let typst_output = format!("#import \"utils.typ\": *\n{}", typst_output);

            let typst_filename = format!("problem-{}.typ", typst_index);
            fs::write(tmp_dir.join(&typst_filename), typst_output)?;
            info!("生成: {}", typst_filename);

            if img_src_dir.exists() && img_src_dir.is_dir() {
                let img_dst_dir = tmp_dir.join("img");
                if !img_dst_dir.exists() {
                    fs::create_dir_all(&img_dst_dir)?;
                }

                process_images_with_unique_ids(&img_src_dir, &img_dst_dir, typst_index)?;
                info!(
                    "处理图片资源: {} -> {}",
                    img_src_dir.display(),
                    img_dst_dir.display()
                );
            }

            problem_pb.inc(1);
        }

        // 处理注意事项文件
        let precaution_path = config.path.join("precaution.md");
        info!("{}", precaution_path.to_string_lossy());
        if precaution_path.exists() {
            info!("处理注意事项文件: {}", precaution_path.display());
            let content = fs::read_to_string(&precaution_path)?;
            let state = MarkdownParserState::new();
            match parse_markdown(state, &content) {
                Ok(ast) => {
                    info!("生成注意事项Typst...");
                    let typst_output = render_typst(&ast, Config::default().with_width(1000000));
                    let typst_output = format!("#import \"utils.typ\": *\n{}", typst_output);
                    fs::write(tmp_dir.join("precaution.typ"), typst_output)?;
                    info!("生成: precaution.typ");
                }
                Err(e) => {
                    warn!("解析注意事项文件失败: {:?}", e);
                }
            }
        } else {
            warn!("未找到注意事项文件");
        }

        // 编译PDF
        info!("开始编译PDF: {}", output_filename);
        let compile_pb = get_context().multiprogress.add(ProgressBar::new_spinner());
        compile_pb.enable_steady_tick(Duration::from_millis(100));
        compile_pb.set_message(format!("编译PDF: {}", output_filename));

        let compile_result = Command::new("typst")
            .arg("compile")
            .arg("--font-path=fonts")
            .arg("main.typ")
            .arg(&output_filename)
            .current_dir(&tmp_dir)
            .output()?;

        compile_pb.finish_and_clear();

        if skip_level == 0 {
            problem_pb.finish_and_clear();
        } else {
            problem_pb.finish_with_message("渲染完成！");
        }

        if compile_result.status.success() {
            info!("编译成功！");

            let pdf_source = tmp_dir.join(&output_filename);
            let pdf_target = statements_dir.join(&output_filename);
            fs::copy(&pdf_source, &pdf_target)?;
            info!("PDF已保存到: {}", pdf_target.display());

            if args.keep_tmp {
                info!("保留临时目录: {}", tmp_dir.display());
            } else {
                fs::remove_dir_all(&tmp_dir)?;
                info!("清理临时目录");
            }
        } else {
            let error_output = String::from_utf8_lossy(&compile_result.stderr);
            error!("编译失败:\n{}", error_output);

            warn!("保留临时目录以供调试: {}", tmp_dir.display());
            return Err("编译过程出错".into());
        }

        // 如果是特定天，处理完后就跳出循环
        if skip_level >= 1 {
            break;
        }
    }

    if skip_level == 0 {
        day_pb.finish_with_message("渲染完成！");
    } else {
        info!("渲染完成！");
    }

    Ok(())
}
