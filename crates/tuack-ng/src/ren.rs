use crate::context::gctx;
use crate::prelude::*;
use clap::Args;
use indexmap::IndexMap;
use indicatif::ProgressBar;
use opener::open;
use std::time::Duration;
use tuack_lib::ren::{
    DateInfo, Problem, ProblemMeta, ProblemType, RenConfig, RenParams, RenProcessor,
    RenderDocument, SupportLanguage,
};
use tuack_ng_parser::parse;
use tuack_utils::assets::FsAssetProvider;
use tuack_utils::plugin::manager::RenderOptions;
use tuack_utils::ren::renderers::{ImageCollector, rewrite_images};
use tuack_utils::ren::template::render_template;

#[derive(Args, Debug)]
#[command(version)]
pub struct RenArgs {
    /// 渲染目标（内置模板名，或插件组件名）
    #[arg(required = true)]
    pub target: String,

    /// 保留临时目录用于调试
    #[arg(long)]
    pub keep_tmp: bool,

    /// 不自动打开渲染成果
    #[arg(short = 's')]
    pub no_auto_open: bool,
}

/// 解析 day -> contest -> 插件模板 覆盖链，得到最终渲染参数。
fn resolve_ren_params(
    config: &ContestConfig,
    day_config: &ContestDayConfig,
    options: &RenderOptions,
) -> RenParams {
    RenParams {
        use_pretest: day_config
            .use_pretest
            .or(config.use_pretest)
            .unwrap_or(options.use_pretest),
        noi_style: day_config
            .noi_style
            .or(config.noi_style)
            .unwrap_or(options.noi_style),
        file_io: day_config
            .file_io
            .or(config.file_io)
            .unwrap_or(options.file_io),
    }
}

/// 构造自洽渲染配置（day -> contest -> template 覆盖链合并）
fn build_ren_config(
    config: &ContestConfig,
    day_config: &ContestDayConfig,
    params: RenParams,
) -> Result<RenConfig> {
    let date = if let (Some(start), Some(end)) = (day_config.start_time, day_config.end_time) {
        Some(DateInfo { start, end })
    } else {
        None
    };

    let mut support_languages = Vec::new();
    for (lang_key, compile_options) in &day_config.compile {
        let language_name = gctx()
            .languages
            .get(lang_key)
            .map(|lang| lang.language.clone())
            .ok_or_else(|| anyhow!("在语言配置中未找到 {}", lang_key))?;
        support_languages.push(SupportLanguage {
            name: language_name,
            compile_options: compile_options.clone(),
        });
    }

    if day_config.name.is_empty() {
        bail!("比赛日 name 不能为空");
    }

    Ok(RenConfig {
        title: config.title.clone(),
        short_title: config.short_title.clone(),
        day_key: day_config.name.clone(),
        dayname: day_config.title.clone(),
        date,
        params,
        support_languages,
    })
}

/// 从 ProblemConfig 提取题目渲染元信息
fn build_problem_meta(problem: &ProblemConfig, day_config: &ContestDayConfig) -> ProblemMeta {
    let submit_filenames = day_config
        .compile
        .keys()
        .map(|lang_key| format!("{}.{}", problem.name, lang_key))
        .collect();

    let point_equal = if problem.runtime.data.is_empty() {
        true
    } else {
        let first = problem.runtime.data[0].score;
        problem.runtime.data.iter().all(|item| item.score == first)
    };

    ProblemMeta {
        name: problem.name.clone(),
        title: problem.title.clone(),
        problem_type: match problem.problem_type {
            tuack_config::ProblemType::Program => ProblemType::Program,
            tuack_config::ProblemType::Output => ProblemType::Output,
            tuack_config::ProblemType::Interactive => ProblemType::Interactive,
        },
        time_limit: Duration::from_secs_f64(problem.time_limit),
        memory_limit: problem.memory_limit,
        testcase: problem.runtime.data.len(),
        point_equal,
        submit_filename: submit_filenames,
    }
}

/// 构造一天的可渲染文档：读题面 -> 模板展开 -> 解析 -> 处理器 -> 图片扫描登记。
fn build_render_document(
    config: &ContestConfig,
    options: &RenderOptions,
    day_config: &ContestDayConfig,
    problem: Option<String>,
    processors: &[Box<dyn RenProcessor>],
    problem_pb: &ProgressBar,
) -> Result<(RenderDocument, FsAssetProvider)> {
    let problems_to_render: IndexMap<String, &ProblemConfig> = match problem {
        Some(ref problem_key) => day_config
            .subconfig
            .get(problem_key)
            .map(|config| {
                info!("渲染指定问题：{}", config.name);
                IndexMap::from([(problem_key.to_string(), config)])
            })
            .context(format!("未找到问题：{}", problem_key))?,
        None => {
            info!("渲染所有问题（共{}个）", day_config.subconfig.len());
            day_config
                .subconfig
                .iter()
                .map(|(k, v)| (k.clone(), v))
                .collect()
        }
    };

    let day_to_render = if problem.is_some() {
        ContestDayConfig {
            subconfig: problems_to_render
                .iter()
                .map(|(k, v)| (k.clone(), (*v).clone()))
                .collect::<IndexMap<_, _>>()
                .into(),
            ..day_config.clone()
        }
    } else {
        day_config.clone()
    };

    let params = resolve_ren_params(config, day_config, options);
    let re = regex::Regex::new(r"<!--[\s\S]*?-->").unwrap();
    let mut assets = FsAssetProvider::new();
    let mut problems = Vec::new();

    for (idx, (_problem_key, problem_config)) in problems_to_render.iter().enumerate() {
        problem_pb.set_message(format!("处理问题：{}", problem_config.name));
        info!("处理问题：{}", problem_config.name);

        let problem_dir = &problem_config.path;
        let statement_path = problem_dir.join("statement.md");
        if !statement_path.exists() {
            bail!("未找到题面文件：{}", statement_path.display());
        }

        // 解析题面同时展开模板，移除注释
        let (content, warnings) = render_template(
            re.replace_all(&fs::read_to_string(&statement_path)?, "")
                .as_ref(),
            problem_config,
            &day_to_render,
            config,
            problem_config.path.clone(),
            params,
        )
        .with_context(|| format!("读取题面文件/展开模板失败：{}", statement_path.display()))?;

        if !warnings.is_empty() {
            let joined = warnings
                .iter()
                .map(|w| format!("  {}", w))
                .collect::<Vec<_>>()
                .join("\n");
            msg_warn!(
                "在解析题目 {} 时产生了警告：\n{}",
                problem_config.name.magenta(),
                joined
            );
        }

        let mut ast = parse(&content);
        for processor in processors {
            let output = processor.process(&ast)?;
            ast = output.ast;
            if !output.warnings.is_empty() {
                let joined = output
                    .warnings
                    .iter()
                    .map(|w| format!("  {}", w))
                    .collect::<Vec<_>>()
                    .join("\n");
                msg_warn!(
                    "处理器在题目 {} 上产生了警告：\n{}",
                    problem_config.name.magenta(),
                    joined
                );
            }
        }

        let (ast, images) = rewrite_images(ast, idx as u64)?;

        assets.register(idx as u64, problem_config.path.clone());

        problems.push(Problem {
            idx: idx as u64,
            meta: build_problem_meta(problem_config, day_config),
            ast,
            images,
        });

        problem_pb.inc(1);
    }

    // 处理注意事项文件
    let precaution_path = config.path.join("precaution.md");
    if !precaution_path.exists() {
        bail!("未找到注意事项文件：{}", precaution_path.display());
    }
    let precaution_ast = parse(&fs::read_to_string(&precaution_path)?);
    if !ImageCollector::collect(&precaution_ast).is_empty() {
        bail!("注意事项不支持图片");
    }
    info!("处理注意事项文件：{}", precaution_path.display());

    let config = build_ren_config(config, day_config, params)?;

    Ok((
        RenderDocument {
            config,
            problems,
            precaution: Some(precaution_ast),
        },
        assets,
    ))
}

fn ren(
    config: &ContestConfig,
    day_config: &ContestDayConfig,
    problem: Option<String>,
    statements_dir: &Path,
    args: &RenArgs,
) -> Result<()> {
    let tmp = Arc::new(
        tempfile::Builder::new()
            .prefix("tuack-ng-ren-")
            .tempdir()
            .context("创建临时目录失败")?,
    );
    let tmp_dir = tmp.path().to_path_buf();
    info!("创建临时目录：{}", tmp_dir.display());

    let problem_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(if problem.is_some() {
            1
        } else {
            day_config.subconfig.len() as u64
        }));
    problem_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let (renderer, options, processors) = match gctx().plugins.renderer(&args.target, tmp.clone()) {
        Ok(triple) => triple,
        Err(e) => {
            problem_pb.finish_with_message("遇到错误，停止处理");
            return Err(e);
        }
    };

    let (doc, assets) = match build_render_document(
        config,
        &options,
        day_config,
        problem,
        &processors,
        &problem_pb,
    ) {
        Ok(pair) => pair,
        Err(e) => {
            problem_pb.finish_with_message("遇到错误，停止处理");
            return Err(e);
        }
    };
    problem_pb.finish_and_clear();

    info!("开始渲染：{}", day_config.name);
    let compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());
    compile_pb.enable_steady_tick(Duration::from_millis(100));
    compile_pb.set_message(format!("渲染：{}", day_config.name));

    let render_result = renderer.render(&doc, Box::new(assets));

    compile_pb.finish_and_clear();

    let (target, files) = match render_result {
        Ok(result) => result,
        Err(e) => {
            msg_error!("渲染失败:\n{:?}", e);
            msg_info!("保留临时目录以供调试：{}", tmp_dir.display());
            // Arc<TempDir> 无 keep()；泄漏一个引用阻止 drop，从而保留目录
            std::mem::forget(tmp.clone());
            bail!("渲染过程出错");
        }
    };

    if let Err(e) = crate::utils::filesystem::write_outputs(statements_dir, files) {
        msg_error!("写入渲染结果失败：{:?}", e);
        msg_info!("保留临时目录以供调试：{}", tmp_dir.display());
        // 同上
        std::mem::forget(tmp.clone());
        bail!("写入渲染结果失败");
    }
    msg_info!("结果已保存到：{}", statements_dir.display());

    if !args.no_auto_open {
        let _ = open(statements_dir.join(target));
    }

    if args.keep_tmp {
        msg_info!("保留临时目录：{}", tmp_dir.display());
        // 同上
        std::mem::forget(tmp.clone());
    } else {
        info!("清理临时目录");
    }

    Ok(())
}

pub fn main(args: RenArgs) -> Result<()> {
    debug!(
        "当前目录：{}",
        dunce::canonicalize(Path::new("."))?.to_string_lossy()
    );

    let Config {
        config,
        location: current_location,
    } = gctx().config.as_ref().context("找不到配置文件")?;

    let statements_dir = match current_location {
        CurrentLocation::Problem(day_name, problem_name) => Path::new(&config.path)
            .join(day_name)
            .join(problem_name)
            .join("statements"),
        CurrentLocation::Day(day_name) => Path::new(&config.path).join(day_name).join("statements"),
        _ => config.path.join("statements"),
    };

    info!("{}", statements_dir.to_string_lossy());
    if !statements_dir.exists() {
        info!("创建题面输出目录：{}", statements_dir.display());
        fs::create_dir(&statements_dir)?;
    }

    if !gctx().plugins.template_exists(&args.target) {
        bail!("没有找到模板 {}", args.target);
    }

    let statements_dir = statements_dir.join(&args.target);
    if !statements_dir.exists() {
        info!(
            "创建 {} 目标输出目录：{}",
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
            let mut failed_days = Vec::new();
            for (day_count, (day_name, day_config)) in config.subconfig.iter().enumerate() {
                day_pb.set_message(format!("处理第 {}/{} 天", day_count, total_days));
                if let Err(e) = ren(config, day_config, None, &statements_dir, &args) {
                    msg_error!("第 {} 天渲染失败：{:?}", day_name, e);
                    failed_days.push(day_name.clone());
                }
                day_pb.inc(1);
            }
            day_pb.finish_with_message("渲染完成！");
            if !failed_days.is_empty() {
                bail!("以下天渲染失败：{}", failed_days.join(", "));
            }
        }
        CurrentLocation::Day(day) => {
            ren(
                config,
                config.subconfig.get(day).unwrap(),
                None,
                &statements_dir,
                &args,
            )?;
        }
        CurrentLocation::Problem(day, problem) => {
            ren(
                config,
                config.subconfig.get(day).unwrap(),
                Some(problem.to_string()),
                &statements_dir,
                &args,
            )?;
        }
        CurrentLocation::None => bail!("没有有效的配置文件"),
    }
    Ok(())
}
