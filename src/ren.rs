use clap::Args;
use log::{error, info, warn};
use markdown_ppp::parser::*;
use markdown_ppp::typst_printer::config::Config;
use markdown_ppp::typst_printer::render_typst;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::{DataJson, Problem};

#[derive(Args, Debug)]
#[command(version)]
pub struct RenArgs {}

pub fn main(args: RenArgs) -> Result<(), Box<dyn std::error::Error>> {
    info!("检查Typst编译环境");

    let typst_check = Command::new("typst").arg("--version").output();

    match typst_check {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                info!("Typst 版本: {}", version.trim());
            } else {
                error!("Typst 命令执行失败，请检查是否已安装");
                return Err("Typst 命令执行失败，请检查是否已安装".into());
            }
        }
        Err(_) => {
            return Err("未找到 typst 命令，请确保已安装并添加到PATH".into());
        }
    }

    let required_files = ["data.json", "main.typ", "utils.typ"];

    for file in required_files {
        if !Path::new(file).exists() {
            error!("缺少必要文件: {}", file);
            return Err(format!("缺少必要文件: {}", file).into());
        }
        info!("文件存在: {}", file);
    }

    let tmp_dir = Path::new("tmp");
    if tmp_dir.exists() {
        info!("清理已存在的 tmp 目录");
        fs::remove_dir_all(tmp_dir)?;
    }
    fs::create_dir(tmp_dir)?;
    info!("创建 tmp 目录");

    info!("复制文件到 tmp 目录");
    fs::copy("main.typ", tmp_dir.join("main.typ"))?;
    fs::copy("utils.typ", tmp_dir.join("utils.typ"))?;
    fs::copy("data.json", tmp_dir.join("data.json"))?;

    if Path::new("fonts").exists() {
        copy_dir_recursive("fonts", tmp_dir.join("fonts"))?;
        info!("复制 fonts 目录");
    }

    let data_content = fs::read_to_string("data.json")?;
    let data: DataJson =
        serde_json::from_str(&data_content).map_err(|e| format!("data.json 格式错误: {}", e))?;
    info!("data.json 格式验证通过，共 {} 个问题", data.problems.len());

    for (idx, problem) in data.problems.iter().enumerate() {
        let markdown_file = format!("{}.md", problem.name);

        if !Path::new(&markdown_file).exists() {
            warn!("未找到问题文件: {}, 跳过", markdown_file);
            continue;
        }

        info!("处理文件: {} -> problem-{}.typ", markdown_file, idx);

        let content = fs::read_to_string(&markdown_file)?;
        let state = MarkdownParserState::new();
        let ast = match parse_markdown(state, &content) {
            Ok(ast) => ast,
            Err(e) => {
                warn!("解析文件 {} 失败: {:?}, 跳过", markdown_file, e);
                continue;
            }
        };

        let typst_output = render_typst(&ast, Config::default().with_width(1000000));
        let typst_output = format!("#import \"utils.typ\": *\n{}", typst_output);

        let typst_filename = format!("problem-{}.typ", idx);
        fs::write(tmp_dir.join(&typst_filename), typst_output)?;
        info!("生成: tmp/{}", typst_filename);
    }

    if Path::new("precaution.md").exists() {
        info!("处理 precaution.md");
        let content = fs::read_to_string("precaution.md")?;
        let state = MarkdownParserState::new();
        let ast = match parse_markdown(state, &content) {
            Ok(ast) => ast,
            Err(e) => {
                warn!("解析 precaution.md 失败: {:?}", e);
                return Err("解析 precaution.md 失败".into());
            }
        };

        let typst_output = render_typst(&ast, Config::default().with_width(1000000));
        let typst_output = format!("#import \"utils.typ\": *\n{}", typst_output);
        fs::write(tmp_dir.join("precaution.typ"), typst_output)?;
        info!("生成: tmp/precaution.typ");
    } else {
        warn!("未找到 precaution.md");
    }

    info!("开始编译...");
    let compile_result = Command::new("typst")
        .arg("compile")
        .arg("--font-path=fonts")
        .arg("main.typ")
        .arg("output.pdf")
        .current_dir(tmp_dir)
        .output()?;

    if compile_result.status.success() {
        info!("编译成功！生成 tmp/output.pdf");

        // 可选：复制 PDF 到当前目录
        fs::copy(tmp_dir.join("output.pdf"), "output.pdf")?;
        info!("PDF 已复制到当前目录: output.pdf");
    } else {
        let error_output = String::from_utf8_lossy(&compile_result.stderr);
        error!("编译失败:");
        error!("{}", error_output);
        return Err("编译过程出错".into());
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
