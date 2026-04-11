use crate::prelude::*;
pub mod manifest;
pub mod processors;
pub mod renderers;
pub mod template;
pub mod unwrap;
pub mod utils;
use crate::ren::processors::process_ast;
use crate::ren::renderers::markdown::MarkdownChecker;
use crate::ren::renderers::markdown::MarkdownCompiler;
use crate::ren::renderers::typst::{TypstChecker, TypstCompiler};
use crate::ren::unwrap::unwrap_template;
use crate::tuack_lib::ren::base::Checker;
use crate::tuack_lib::ren::base::Compiler;
use clap::Args;
use colored::Colorize;
use indexmap::IndexMap;
use manifest::TargetType;
use manifest::TemplateManifest;
use markdown_ppp::ast::Document;
use markdown_ppp::parser::*;
use opener::open;
use std::time::Duration;

use template::render_template;
use utils::{process_image_urls, process_images_with_unique_ids};

use crate::utils::filesystem::copy_dir_recursive;

use indicatif::ProgressBar;

use crate::context;
use crate::context::{CurrentLocation, gctx};

#[derive(Args, Debug)]
#[command(version)]
pub struct RenArgs {
    /// 渲染目标模板
    #[arg(required = true)]
    pub target: String,

    /// 保留临时目录用于调试
    #[arg(long)]
    pub keep_tmp: bool,

    /// 不自动打开渲染成果
    #[arg(short = 's')]
    pub no_auto_open: bool,
}

pub enum RenderQueue {
    Problem(Document, Box<ProblemConfig>),
    Precaution(Document),
}

fn ren(
    config: &ContestConfig,
    manifest: &TemplateManifest,
    day_config: &ContestDayConfig,
    problem: Option<String>,
    statements_dir: &PathBuf,
    args: &RenArgs,
    is_contest_level: bool,
) -> Result<()> {
    let tmp_dir = statements_dir.join("tmp");

    // 清理已存在的临时目录
    if tmp_dir.exists() {
        info!("清理已存在的临时目录: {}", tmp_dir.display());
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir(&tmp_dir)?;
    info!("创建临时目录: {}", tmp_dir.display());

    // info!("复制模板文件到临时目录");
    // copy_dir_recursive(template_dir, &tmp_dir)?;

    // let tmp_font_dir = tmp_dir.join("fonts");
    // if tmp_font_dir.exists() {
    //     fs::remove_dir_all(&tmp_font_dir)?;
    // }

    // info!("复制字体文件到临时目录");
    // copy_dir_recursive(fonts_dir, &tmp_font_dir)?;

    unwrap_template(manifest, &tmp_dir)?;

    let checker: Box<dyn Checker> = match manifest.target {
        TargetType::Typst => Box::new(TypstChecker::new(tmp_dir.clone())),
        TargetType::Markdown => Box::new(MarkdownChecker::new(tmp_dir.clone())),
    };
    if let Err(e) = checker.check_compiler() {
        bail!(e.context("渲染器检查未通过"));
    }

    // 获取要渲染的问题
    let problems_to_render: IndexMap<String, &ProblemConfig> = match problem {
        Some(ref problem_key) => day_config
            .subconfig
            .get(problem_key)
            .map(|config| {
                info!("渲染指定问题: {}", config.name);
                IndexMap::from([(problem_key.to_string(), config)])
            })
            .context(format!("未找到问题: {}", problem_key))?,
        None => {
            info!("渲染所有问题（共{}个）", day_config.subconfig.len());
            day_config
                .subconfig
                .iter()
                .map(|(k, v)| (k.clone(), v))
                .collect()
        }
    };

    // 添加问题级别进度条
    let problem_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(problems_to_render.len() as u64));

    problem_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let mut renderqueue = Vec::<RenderQueue>::new();

    let day_to_render = if problem.is_some() {
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

    for (_problem_key, problem_config) in problems_to_render.iter() {
        problem_pb.set_message(format!("处理问题: {}", problem_config.name));
        info!("处理问题: {}", problem_config.name);

        // 题面文件路径
        let problem_dir = &problem_config.path;
        let statement_path = problem_dir.join("statement.md");

        if !statement_path.exists() {
            msg_error!("未找到题面文件: {}", statement_path.display());
            problem_pb.finish_with_message("遇到错误，停止处理");
            bail!("未找到题面文件: {}", statement_path.display());
        }

        // 解析题面同时展开模板
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
                msg_error!(
                    "读取题面文件/展开模板 {} 失败: {:?}",
                    statement_path.display(),
                    e
                );
                problem_pb.finish_with_message("遇到错误，停止处理");
                bail!("解析题面文件失败");
            }
        };

        let state = MarkdownParserState::new();
        let mut ast = match parse_markdown(state, &content) {
            Ok(ast) => ast,
            Err(e) => {
                msg_error!("解析题面文件 {} 失败: {:?}", statement_path.display(), e);
                problem_pb.finish_with_message("遇到错误，停止处理");
                bail!("解析题面文件失败");
            }
        };

        ast = process_ast(&mut ast, &manifest.processor)?;

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

            process_images_with_unique_ids(&img_src_dir, &img_dst_dir)?;
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
                msg_warn!("解析注意事项文件失败: {:?}", e);
            }
        }
    } else {
        msg_warn!("未找到注意事项文件");
    }

    problem_pb.finish_and_clear();

    // 编译PDF
    info!("开始编译: {}", day_config.name);
    let compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());
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

    if is_contest_level {
        problem_pb.finish_and_clear();
    } else {
        problem_pb.finish_with_message("渲染完成！");
    }

    match compile_result {
        Ok(output_filename) => {
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
            msg_info!("结果已保存到: {}", target.display());

            if !args.no_auto_open {
                open(target)?;
            }

            if args.keep_tmp {
                msg_info!("保留临时目录: {}", tmp_dir.display());
            } else {
                fs::remove_dir_all(&tmp_dir)?;
                info!("清理临时目录");
            }
        }
        Err(e) => {
            msg_error!("编译失败:\n{:?}", e);

            msg_info!("保留临时目录以供调试: {}", tmp_dir.display());
            bail!("编译过程出错");
        }
    }

    Ok(())
}

pub fn main(args: RenArgs) -> Result<()> {
    debug!(
        "当前目录: {}",
        dunce::canonicalize(Path::new("."))?.to_string_lossy()
    );

    let (config, current_location) = gctx().config.as_ref().context("找不到配置文件")?;

    let manifest_file = context::gctx().assets_dirs.iter().find(|dir| {
        let subdir = dir.join("templates").join(format!("{}.json", args.target));
        subdir.exists() && subdir.is_file()
    });

    let manifest_file = match manifest_file {
        Some(dir) => {
            info!(
                "找到清单文件: {}",
                dir.join("templates")
                    .join(format!("{}.json", args.target))
                    .to_string_lossy()
            );
            dir.join("templates").join(format!("{}.json", args.target))
        }
        None => {
            msg_error!("没有找到模板 {}", args.target);
            bail!("没有找到模板 {}", args.target);
        }
    };

    // let fonts_dir = context::gctx().assets_dirs.iter().find(|dir| {
    //     let subdir = dir.join("templates").join("fonts");
    //     subdir.exists() && subdir.is_dir()
    // });

    // let fonts_dir = match fonts_dir {
    //     Some(dir) => {
    //         info!(
    //             "找到字体目录: {}",
    //             dir.join("templates").join("fonts").to_string_lossy()
    //         );
    //         dir.join("templates").join("fonts")
    //     }
    //     None => {
    //         msg_error!("没有找到字体目录");
    //         bail!("致命错误: 没有找到模板 {}", args.target);
    //     }
    // };

    let manifest = serde_json::from_str::<TemplateManifest>(&fs::read_to_string(&manifest_file)?)?;

    // let checker: Box<dyn Checker> = match manifest.target {
    //     TargetType::Typst => Box::new(TypstChecker::new(template_dir.to_path_buf())),
    //     TargetType::Markdown => Box::new(MarkdownChecker::new(template_dir.to_path_buf())),
    // };
    // if let Err(e) = checker.check_compiler() {
    //     bail!(e.context("渲染器检查未通过"));
    // }

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

    match &current_location {
        CurrentLocation::Root => {
            let total_days = config.subconfig.len();
            let day_pb = gctx()
                .multiprogress
                .add(ProgressBar::new(total_days as u64));
            day_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.green/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );
            for (day_count, (_day_name, day_config)) in config.subconfig.iter().enumerate() {
                day_pb.set_message(format!("处理第 {}/{} 天", day_count, total_days));
                ren(
                    &config,
                    &manifest,
                    &day_config,
                    None,
                    &statements_dir,
                    &args,
                    true,
                )?;
                day_pb.inc(1);
            }
            day_pb.finish_with_message("渲染完成！");
        }
        CurrentLocation::Day(day) => {
            ren(
                &config,
                &manifest,
                &config.subconfig.get(day).unwrap(),
                None,
                &statements_dir,
                &args,
                false,
            )?;
        }
        CurrentLocation::Problem(day, problem) => {
            ren(
                &config,
                &manifest,
                &config.subconfig.get(day).unwrap(),
                Some(problem.to_string()),
                &statements_dir,
                &args,
                false,
            )?;
        }
        CurrentLocation::None => bail!("没有有效的配置文件"),
    }
    Ok(())
}
