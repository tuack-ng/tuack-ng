use clap::Args;
use log::{debug, error, info, warn};
use markdown_ppp::ast_transform::Transform;
use markdown_ppp::parser::*;
use markdown_ppp::typst_printer::config::Config;
use markdown_ppp::typst_printer::render_typst;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;
use std::{fs, path::PathBuf};

use crate::{
    config::{
        ContestConfig, ContestDayConfig, DataJson, DateInfo, Problem, SupportLanguage,
        TemplateManifest, load_config,
    },
    context,
};

#[derive(Args, Debug)]
#[command(version)]
pub struct RenArgs {
    /// 渲染目标模板
    #[arg(required = true)]
    target: String,

    /// 要渲染的天的名称（可选，如果不指定则渲染所有天）
    #[arg(short, long)]
    day: Option<String>,

    /// 保留临时目录用于调试
    #[arg(long)]
    keep_tmp: bool,
}

pub fn main(args: RenArgs) -> Result<(), Box<dyn std::error::Error>> {
    debug!(
        "当前目录: {}",
        Path::new(".").canonicalize()?.to_string_lossy()
    );
    let config = load_config(Path::new("."))?;

    let template_dir = context::get_context().template_dirs.iter().find(|dir| {
        let subdir = dir.join(&args.target);
        subdir.exists() && subdir.is_dir()
    });

    let template_dir = match template_dir {
        Some(dir) => {
            info!("找到模板目录: {}", dir.join(&args.target).to_string_lossy());
            dir.join(&args.target)
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
    info!("{}", &statements_dir.to_string_lossy());
    if !statements_dir.exists() {
        fs::create_dir(&statements_dir)?;
        info!("创建题面输出目录: {}", statements_dir.display());
    }

    // 过滤要渲染的天
    let days_to_render: Vec<&ContestDayConfig> = if let Some(day_name) = &args.day {
        match config.subconfig.iter().find(|d| d.name == *day_name) {
            Some(day) => {
                info!("渲染指定天: {}", day_name);
                vec![day]
            }
            None => {
                error!("未找到天: {}", day_name);
                return Err(format!("未找到天: {}", day_name).into());
            }
        }
    } else {
        info!("渲染所有天（共{}个）", config.subconfig.len());
        config.subconfig.iter().collect()
    };

    for day in days_to_render {
        info!("开始渲染天: {}", day.name);
        render_day(&config, day, &template_dir, &statements_dir, &args)?;
    }

    info!("所有天的题面渲染完成！");
    Ok(())
}

fn generate_data_json(
    contest_config: &ContestConfig,
    day_config: &ContestDayConfig,
) -> Result<DataJson, Box<dyn std::error::Error>> {
    // 构建问题列表
    let mut problems = Vec::new();

    for problem_config in &day_config.subconfig {
        let problem = Problem {
            name: problem_config.name.clone(),
            title: problem_config.title.clone(),
            dir: problem_config.name.clone(), // 假设目录名就是问题名
            exec: problem_config.name.clone(), // 默认值，你可能需要从配置文件读取
            input: problem_config.name.clone() + ".in",
            output: problem_config.name.clone() + ".out",
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
            point_equal: "是".to_string(),
            submit_filename: vec![format!("{}.cpp", problem_config.name)], // 默认值
        };
        problems.push(problem);
    }

    // 构建支持的语言列表
    // 注意：ContestConfig中没有support_languages字段，这里使用默认值
    let support_languages = vec![SupportLanguage {
        name: "C++".to_string(),
        compile_options: day_config.compile.cpp.clone(),
    }];

    // 创建日期信息
    let date = DateInfo {
        start: day_config.start_time,
        end: day_config.end_time,
    };

    // 读取模板目录中的清单文件以获取默认值
    let manifest_path = context::get_context().template_dirs.iter().find_map(|dir| {
        let manifest_file = dir.join("noi").join("manifest.json");
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
        images: Vec::new(),
    })
}

fn render_day(
    contest_config: &ContestConfig,
    day_config: &ContestDayConfig,
    template_dir: &PathBuf,
    output_dir: &PathBuf,
    args: &RenArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let day_output_dir = output_dir.join(&day_config.name);
    if !day_output_dir.exists() {
        fs::create_dir(&day_output_dir)?;
        info!("创建天输出目录: {}", day_output_dir.display());
    }

    let tmp_dir = day_output_dir.join("tmp");
    if tmp_dir.exists() {
        info!("清理已存在的临时目录: {}", tmp_dir.display());
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir(&tmp_dir)?;
    info!("创建临时目录: {}", tmp_dir.display());

    info!("复制模板文件到临时目录");
    copy_dir_recursive(template_dir, &tmp_dir)?;

    // 生成并写入 data.json
    let data_json = generate_data_json(contest_config, day_config)?;
    let data_json_str = serde_json::to_string_pretty(&data_json)?;
    fs::write(tmp_dir.join("data.json"), data_json_str)?;
    info!("生成 data.json");

    // 复制问题资源和题面
    for (idx, problem) in day_config.subconfig.iter().enumerate() {
        info!(
            "处理问题 {}/{}: {}",
            idx + 1,
            day_config.subconfig.len(),
            problem.name
        );

        // 题面文件路径
        let problem_dir = &problem.path;
        let statement_path = problem_dir.join("statement.md");

        if !statement_path.exists() {
            error!("未找到题面文件: {}", statement_path.display());
            return Err(format!("未找到题面文件: {}", statement_path.display()).into());
        }

        // 解析题面
        let content = fs::read_to_string(&statement_path)?;
        let state = MarkdownParserState::new();
        let mut ast = match parse_markdown(state, &content) {
            Ok(ast) => ast,
            Err(e) => {
                error!("解析题面文件 {} 失败: {:?}", statement_path.display(), e);
                return Err("解析题面文件失败".into());
            }
        };

        // 修改图片路径，将相对路径替换为唯一ID路径
        let img_src_dir = problem_dir.join("img");
        if img_src_dir.exists() && img_src_dir.is_dir() {
            ast = ast.transform_image_urls(|url| {
                // 如果URL是相对路径且指向img目录，则替换为唯一ID路径
                if url.starts_with("./img/") || url.starts_with("img/") {
                    // 提取文件名
                    let filename = Path::new(&url)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&url);

                    // 计算文件的SHA256哈希值
                    let img_path = img_src_dir.join(filename);
                    if img_path.exists() {
                        match std::fs::File::open(&img_path) {
                            Ok(mut file) => {
                                let mut hasher = Sha256::new();
                                if std::io::copy(&mut file, &mut hasher).is_ok() {
                                    let hash = hasher.finalize();
                                    let hash_hex = format!("{:x}", hash);

                                    // 获取文件扩展名
                                    let extension = img_path
                                        .extension()
                                        .and_then(|ext| ext.to_str())
                                        .unwrap_or("");

                                    // 生成唯一ID路径
                                    if extension.is_empty() {
                                        format!("img/{}", hash_hex)
                                    } else {
                                        format!("img/{}.{}", hash_hex, extension)
                                    }
                                } else {
                                    url // 如果计算哈希失败，保持原URL
                                }
                            }
                            Err(_) => url, // 如果打开文件失败，保持原URL
                        }
                    } else {
                        url // 如果文件不存在，保持原URL
                    }
                } else {
                    warn!(
                        "图片 url 不合法: {}, 不支持使用在 img/ 以外的图片, 可能会产生问题。",
                        url
                    );
                    url
                }
            });
        }

        // 生成Typst内容
        let typst_output = render_typst(&ast, Config::default().with_width(1000000));
        let typst_output = format!("#import \"utils.typ\": *\n{}", typst_output);

        // 写入Typst文件
        let typst_filename = format!("problem-{}.typ", idx);
        fs::write(tmp_dir.join(&typst_filename), typst_output)?;
        info!("生成: {}", typst_filename);

        // 处理图片资源
        let img_src_dir = problem_dir.join("img");
        if img_src_dir.exists() && img_src_dir.is_dir() {
            let img_dst_dir = tmp_dir.join("img");
            if !img_dst_dir.exists() {
                fs::create_dir_all(&img_dst_dir)?;
            }

            // 为每个图片分配唯一ID并复制
            process_images_with_unique_ids(&img_src_dir, &img_dst_dir, idx)?;
            info!(
                "处理图片资源: {} -> {}",
                img_src_dir.display(),
                img_dst_dir.display()
            );
        }
    }

    // 处理注意事项文件
    let precaution_path = contest_config.path.join("precaution.md");
    info!("{}", precaution_path.to_string_lossy());
    if precaution_path.exists() {
        info!("处理注意事项文件: {}", precaution_path.display());
        let content = fs::read_to_string(&precaution_path)?;
        let state = MarkdownParserState::new();
        match parse_markdown(state, &content) {
            Ok(ast) => {
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
    info!("开始编译PDF...");
    let compile_result = Command::new("typst")
        .arg("compile")
        .arg("--font-path=fonts")
        .arg("main.typ")
        .arg(format!("{}.pdf", day_config.name))
        .current_dir(&tmp_dir)
        .output()?;

    if compile_result.status.success() {
        info!("编译成功！");

        // 复制PDF到输出目录
        let pdf_source = tmp_dir.join(format!("{}.pdf", day_config.name));
        let pdf_target = day_output_dir.join(format!("{}.pdf", day_config.name));
        fs::copy(&pdf_source, &pdf_target)?;
        info!("PDF已保存到: {}", pdf_target.display());

        // 根据keep_tmp参数决定是否清理临时目录
        if args.keep_tmp {
            info!("保留临时目录: {}", tmp_dir.display());
        } else {
            fs::remove_dir_all(&tmp_dir)?;
            info!("清理临时目录");
        }
    } else {
        let error_output = String::from_utf8_lossy(&compile_result.stderr);
        error!("编译失败:\n{}", error_output);

        // 保留临时目录以供调试
        warn!("保留临时目录以供调试: {}", tmp_dir.display());
        return Err("编译过程出错".into());
    }

    Ok(())
}

// 为图片分配唯一ID并复制的函数
fn process_images_with_unique_ids(
    src_dir: &Path,
    dst_dir: &Path,
    _problem_idx: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if !dst_dir.exists() {
        fs::create_dir_all(dst_dir)?;
    }

    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let src_path = entry.path();

        if src_path.is_file() {
            // 计算文件的SHA256哈希值
            let mut file = std::fs::File::open(&src_path)?;
            let mut hasher = Sha256::new();
            std::io::copy(&mut file, &mut hasher)?;
            let hash = hasher.finalize();
            let hash_hex = format!("{:x}", hash);

            // 获取文件扩展名
            let extension = src_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");

            // 生成唯一ID: sha256.extension
            let unique_filename = if extension.is_empty() {
                hash_hex
            } else {
                format!("{}.{}", hash_hex, extension)
            };
            let dst_path = dst_dir.join(unique_filename);

            // 复制文件
            fs::copy(&src_path, &dst_path)?;
            info!("复制图片: {} -> {}", src_path.display(), dst_path.display());
        }
    }

    Ok(())
}

// 递归复制目录的辅助函数
fn copy_dir_recursive<P: AsRef<Path>, Q: AsRef<Path>>(
    src: P,
    dst: Q,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
