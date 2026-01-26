pub mod renderers;
pub mod template;
pub mod utils;
use crate::config::ContestDayConfig;
use crate::config::ProblemConfig;
use crate::config::TargetType;
use crate::config::TemplateManifest;
use crate::ren::renderers::base::Checker;
use crate::ren::renderers::base::Compiler;
use crate::ren::renderers::markdown::MarkdownChecker;
use crate::ren::renderers::markdown::MarkdownCompiler;
use crate::ren::renderers::typst::{TypstChecker, TypstCompiler};
use clap::Args;
use indexmap::IndexMap;
use log::{debug, error, info, warn};
use markdown_ppp::ast::Document;
use markdown_ppp::parser::*;
use std::fs;
use std::path::Path;
use std::time::Duration;

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

pub enum RenderQueue {
    Problem(Document, Box<ProblemConfig>),
    Precaution(Document),
}

pub fn main(args: RenArgs) -> Result<(), Box<dyn std::error::Error>> {
    debug!(
        "当前目录: {}",
        Path::new(".").canonicalize()?.to_string_lossy()
    );

    let (config, current_location) = get_context().config.as_ref().ok_or("找不到配置文件")?;

    // 根据当前位置确定skip_level和目标配置的键
    let (skip_level, target_day_key, target_problem_key) = match current_location {
        CurrentLocation::Problem(day_name, problem_name) => {
            (2, Some(day_name.as_str()), Some(problem_name.as_str()))
        }
        CurrentLocation::Day(day_name) => (1, Some(day_name.as_str()), None),
        _ => (0, None, None),
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

    let fonts_dir = context::get_context().assets_dirs.iter().find(|dir| {
        let subdir = dir.join("templates").join("fonts");
        subdir.exists() && subdir.is_dir()
    });

    let fonts_dir = match fonts_dir {
        Some(dir) => {
            info!(
                "找到字体目录: {}",
                dir.join("templates").join("fonts").to_string_lossy()
            );
            dir.join("templates").join("fonts")
        }
        None => {
            error!("没有找到字体目录");
            return Err(format!("致命错误: 没有找到模板 {}", args.target).into());
        }
    };

    let manifest = {
        let manifest_file = template_dir.join("manifest.json");
        if manifest_file.exists() {
            let manifest_content = fs::read_to_string(&manifest_file)?;
            serde_json::from_str::<TemplateManifest>(&manifest_content)?
        } else {
            error!("找不到清单文件: {}", manifest_file.display());
            return Err("致命错误: 找不到清单文件".into());
        }
    };

    let checker: Box<dyn Checker> = match manifest.target {
        TargetType::Typst => Box::new(TypstChecker::new(template_dir.to_path_buf())),
        TargetType::Markdown => Box::new(MarkdownChecker::new(template_dir.to_path_buf())),
    };
    checker.check_compiler()?;

    let statements_dir = match current_location {
        CurrentLocation::Problem(day_name, problem_name) => Path::new(&config.path)
            .join(day_name)
            .join(problem_name)
            .join("statements"),
        CurrentLocation::Day(day_name) => Path::new(&config.path).join(day_name).join("statements"),
        _ => config.path.join("statements"),
    };

    info!("{}", &statements_dir.to_string_lossy());
    if !statements_dir.exists() {
        info!("创建题面输出目录: {}", statements_dir.display());
        fs::create_dir(&statements_dir)?;
    }

    let statements_dir = statements_dir.join(&args.target);
    if !statements_dir.exists() {
        info!(
            "创建 {} 目标输出目录: {}",
            args.target,
            statements_dir.display()
        );
        fs::create_dir(&statements_dir)?;
    }

    // 获取要处理的天配置
    let days_vec;
    let days_to_process: Vec<(&String, &ContestDayConfig)> = if let Some(day_key) = target_day_key {
        let day_config = config
            .subconfig
            .get(day_key)
            .ok_or_else(|| format!("未找到天配置: {}", day_key))?;
        let actual_key = config
            .subconfig
            .keys()
            .find(|k| k.as_str() == day_key)
            .ok_or_else(|| format!("未找到天配置键: {}", day_key))?;
        days_vec = vec![(actual_key, day_config)];
        days_vec.iter().map(|(k, v)| (*k, *v)).collect()
    } else {
        // 所有天
        config.subconfig.iter().collect()
    };

    let total_days = days_to_process.len();

    // 添加天级别进度条（仅当需要处理多天时显示）
    let day_pb = if skip_level >= 1 {
        get_context().multiprogress.add(ProgressBar::new(0))
    } else {
        let pb = get_context()
            .multiprogress
            .add(ProgressBar::new(total_days as u64));
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("  [{bar:40.green/blue}] {msg}")
                .unwrap()
                .progress_chars("=> "),
        );
        pb
    };

    let mut day_count = 0;
    for (_day_key, day_config) in days_to_process {
        day_count += 1;
        if skip_level < 1 {
            day_pb.set_message(format!("处理第 {}/{} 天", day_count, total_days));
        }
        info!("处理天: {}", day_config.name);

        if !statements_dir.exists() {
            fs::create_dir(&statements_dir)?;
            info!("创建输出目录: {}", statements_dir.display());
        }

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

        let tmp_font_dir = tmp_dir.join("fonts");
        if tmp_font_dir.exists() {
            fs::remove_dir_all(&tmp_font_dir)?;
        }

        info!("复制字体文件到临时目录");
        copy_dir_recursive(&fonts_dir, &tmp_font_dir)?;

        // 获取要渲染的问题
        let problems_to_render: IndexMap<String, &ProblemConfig> =
            if let Some(problem_key) = target_problem_key {
                // 特定问题：直接通过键获取
                day_config
                    .subconfig
                    .get(problem_key)
                    .map(|problem_config| {
                        info!("渲染指定问题: {}", problem_config.name);
                        let mut map = IndexMap::new();
                        map.insert(problem_key.to_string(), problem_config);
                        map
                    })
                    .ok_or_else(|| {
                        error!("未找到问题: {}", problem_key);
                        format!("未找到问题: {}", problem_key)
                    })?
            } else {
                // 所有问题
                info!("渲染所有问题（共{}个）", day_config.subconfig.len());
                day_config
                    .subconfig
                    .iter()
                    .map(|(k, v)| (k.clone(), v))
                    .collect()
            };

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

        let mut renderqueue = Vec::<RenderQueue>::new();

        let day_to_render = if skip_level >= 2 {
            ContestDayConfig {
                subconfig: problems_to_render
                    .iter()
                    .map(|(k, v)| (k.clone(), (*v).clone()))
                    .collect(),
                ..day_config.clone()
            }
        } else {
            day_config.clone()
        };

        for (problem_key, problem_config) in problems_to_render.iter() {
            // 计算问题索引
            let typst_index = if skip_level >= 2 {
                0
            } else {
                day_config
                    .subconfig
                    .keys()
                    .position(|k| k == problem_key)
                    .unwrap_or(0)
            };

            problem_pb.set_message(format!("处理问题: {}", problem_config.name));
            info!("处理问题: {}", problem_config.name);

            // 题面文件路径
            let problem_dir = &problem_config.path;
            let statement_path = problem_dir.join("statement.md");

            if !statement_path.exists() {
                error!("未找到题面文件: {}", statement_path.display());
                problem_pb.finish_with_message("遇到错误，停止处理");
                return Err(format!("未找到题面文件: {}", statement_path.display()).into());
            }

            // 解析题面顺便展开模板
            let content = match render_template(
                &fs::read_to_string(&statement_path)?,
                problem_config,
                &day_to_render,
                config,
                problem_config.path.clone(),
                manifest.clone(),
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

            renderqueue.push(RenderQueue::Problem(
                ast,
                Box::new((*problem_config).clone()),
            ));

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
                    renderqueue.push(RenderQueue::Precaution(ast));
                }
                Err(e) => {
                    warn!("解析注意事项文件失败: {:?}", e);
                }
            }
        } else {
            warn!("未找到注意事项文件");
        }

        // 编译PDF
        info!("开始编译: {}", day_config.name);
        let compile_pb = get_context().multiprogress.add(ProgressBar::new_spinner());
        compile_pb.enable_steady_tick(Duration::from_millis(100));
        compile_pb.set_message(format!("编译: {}", day_config.name));

        let compiler: Box<dyn Compiler> = match manifest.target {
            TargetType::Typst => Box::new(TypstCompiler::new(
                config.clone(),
                day_to_render,
                tmp_dir.clone(),
                renderqueue,
                manifest.clone(),
            )),
            TargetType::Markdown => Box::new(MarkdownCompiler::new(
                config.clone(),
                day_to_render,
                tmp_dir.clone(),
                renderqueue,
                manifest.clone(),
            )),
        };

        let compile_result = compiler.compile();

        compile_pb.finish_and_clear();

        if skip_level == 0 {
            problem_pb.finish_and_clear();
        } else {
            problem_pb.finish_with_message("渲染完成！");
        }

        if let Ok(output_filename) = compile_result {
            info!("编译成功！");

            let source = tmp_dir.join(&output_filename);
            let target = if output_filename.is_file() {
                statements_dir.join(output_filename.file_name().unwrap())
            } else {
                statements_dir.clone()
            };
            if output_filename.is_file() {
                fs::copy(&source, &target)?;
            } else {
                copy_dir_recursive(&source, &target)?;
            }
            info!("PDF已保存到: {}", target.display());

            if args.keep_tmp {
                info!("保留临时目录: {}", tmp_dir.display());
            } else {
                fs::remove_dir_all(&tmp_dir)?;
                info!("清理临时目录");
            }
        } else {
            let error_output = &compile_result.err().unwrap().to_string();
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
