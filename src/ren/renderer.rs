use crate::config::{ContestConfig, ContestDayConfig};
use log::{error, info, warn};
use markdown_ppp::ast_transform::Transform;
use markdown_ppp::parser::*;
use markdown_ppp::typst_printer::config::Config;
use markdown_ppp::typst_printer::render_typst;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::{fs, path::PathBuf};

use super::data_json::generate_data_json;
use super::template::render_template;
use super::utils::{copy_dir_recursive, process_images_with_unique_ids};

pub fn render_day(
    contest_config: &ContestConfig,
    day_config: &ContestDayConfig,
    template_dir: &PathBuf,
    output_dir: &Path,
    args: &super::cli::RenArgs,
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
    let multi = &crate::context::get_context().multiprogress;
    let problem_pb = multi.add(indicatif::ProgressBar::new(
        day_config.subconfig.len() as u64
    ));
    problem_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    for (idx, problem) in day_config.subconfig.iter().enumerate() {
        problem_pb.set_message(format!("处理问题: {}", problem.name));
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
            problem_pb.finish_with_message("遇到错误，停止处理");
            return Err(format!("未找到题面文件: {}", statement_path.display()).into());
        }

        // 解析题面顺便展开模板
        let content = match render_template(
            &fs::read_to_string(&statement_path)?,
            problem,
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

        problem_pb.set_message(format!("生成Typst: {}", problem.name));
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

        problem_pb.inc(1);
    }
    problem_pb.finish_and_clear();

    // 处理注意事项文件
    let precaution_path = contest_config.path.join("precaution.md");
    info!("{}", precaution_path.to_string_lossy());
    if precaution_path.exists() {
        info!("处理注意事项文件: {}", precaution_path.display());
        let content = fs::read_to_string(&precaution_path)?;
        let state = MarkdownParserState::new();
        match parse_markdown(state, &content) {
            Ok(ast) => {
                problem_pb.set_message("生成注意事项Typst...");
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
    let multi = &crate::context::get_context().multiprogress;
    let compile_pb = multi.add(indicatif::ProgressBar::new_spinner());
    compile_pb.enable_steady_tick(Duration::from_millis(100));

    compile_pb.set_message(format!("编译PDF: {}", day_config.name));
    let compile_result = Command::new("typst")
        .arg("compile")
        .arg("--font-path=fonts")
        .arg("main.typ")
        .arg(format!("{}.pdf", day_config.name))
        .current_dir(&tmp_dir)
        .output()?;
    compile_pb.finish_and_clear();

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
