use super::nom;
use crate::config::ProblemConfig;
use crate::ren::nom::DotCall;
use log::{debug, error, info, warn};
use markdown_ppp::ast::HeadingKind::Atx;
use markdown_ppp::ast::{Block, Heading, Inline};
use markdown_ppp::ast::{CodeBlock, Document};
use markdown_ppp::ast_transform::{ExpandWith, macro_expansion::MacroTransformer};
use std::rc::Rc;
use std::{fs, path::Path, path::PathBuf};

/// 处理 sample_file 宏（生成文件引用片段）
fn handle_sample_macro(call: &DotCall, problem: &ProblemConfig, base_path: &Path) -> Vec<Block> {
    debug!("处理 sample_file 宏: {:?}", call);

    // 解析样本ID
    let sample_id: u32 = match call.arguments.as_deref() {
        Some(args) => match args.parse() {
            Ok(id) => {
                debug!("解析到样本ID: {}", id);
                id
            }
            Err(e) => {
                warn!("无效的样本ID: {} -> {:?}", args, e);
                return vec![Block::Paragraph(vec![Inline::Text(format!(
                    "无效的样本ID: {}",
                    args
                ))])];
            }
        },
        None => {
            warn!("sample_file 宏缺少参数");
            return vec![Block::Paragraph(vec![Inline::Text(
                "sample_file 宏需要参数，例如: sample_file(1)".to_string(),
            )])];
        }
    };

    // 查找样本配置
    let sample_item = match problem.samples.iter().find(|s| s.id == sample_id) {
        Some(item) => {
            debug!("找到样本配置: {:?}", item);
            item
        }
        None => {
            warn!("未找到样本ID: {}", sample_id);
            return vec![Block::Paragraph(vec![Inline::Text(format!(
                "未找到样本 {}",
                sample_id
            ))])];
        }
    };

    // 检查文件是否存在
    let input_exists = sample_item.input.as_ref().is_some_and(|input_file| {
        let input_path = base_path.join("sample").join(input_file);
        let exists = input_path.exists();
        if !exists {
            warn!("输入文件不存在: {:?}", input_path);
        }
        exists
    });

    let output_exists = sample_item.output.as_ref().is_some_and(|output_file| {
        let output_path = base_path.join("sample").join(output_file);
        let exists = output_path.exists();
        if !exists {
            warn!("输出文件不存在: {:?}", output_path);
        }
        exists
    });

    // 读取文件内容
    let sample_in = if input_exists {
        match &sample_item.input {
            Some(input_file) => {
                let input_path = base_path.join("sample").join(input_file);
                debug!("读取输入文件: {:?}", input_path);
                match fs::read_to_string(&input_path) {
                    Ok(content) => Some(content),
                    Err(e) => {
                        error!("读取输入文件失败: {:?} -> {}", input_path, e);
                        None
                    }
                }
            }
            None => None,
        }
    } else {
        warn!("输入文件不存在，跳过读取");
        None
    };

    let sample_out = if output_exists {
        match &sample_item.output {
            Some(output_file) => {
                let output_path = base_path.join("sample").join(output_file);
                debug!("读取输出文件: {:?}", output_path);
                match fs::read_to_string(&output_path) {
                    Ok(content) => Some(content),
                    Err(e) => {
                        error!("读取输出文件失败: {:?} -> {}", output_path, e);
                        None
                    }
                }
            }
            None => None,
        }
    } else {
        warn!("输出文件不存在，跳过读取");
        None
    };

    // 生成结果块
    let mut blocks = Vec::new();

    // 添加输入标题和内容
    blocks.push(Block::Heading(Heading {
        kind: Atx(2),
        content: vec![Inline::Text(format!("样例 {} 输入", sample_id))],
    }));

    if let Some(input_content) = sample_in {
        blocks.push(Block::CodeBlock(CodeBlock {
            kind: markdown_ppp::ast::CodeBlockKind::Fenced {
                info: Some("txt".to_string()),
            },
            literal: input_content,
        }));
    } else {
        blocks.push(Block::Paragraph(vec![Inline::Text(
            "输入文件不存在或读取失败".to_string(),
        )]));
    }

    // 添加输出标题和内容
    blocks.push(Block::Heading(Heading {
        kind: Atx(2),
        content: vec![Inline::Text(format!("样例 {} 输出", sample_id))],
    }));

    if let Some(output_content) = sample_out {
        blocks.push(Block::CodeBlock(CodeBlock {
            kind: markdown_ppp::ast::CodeBlockKind::Fenced {
                info: Some("txt".to_string()),
            },
            literal: output_content,
        }));
    } else {
        blocks.push(Block::Paragraph(vec![Inline::Text(
            "输出文件不存在或读取失败".to_string(),
        )]));
    }

    info!("成功生成样例 {} 的完整展示", sample_id);
    blocks
}

/// 处理 sample_file 宏 - 生成文件引用片段
/// 格式: sample_file(1) -> "见选手目录下的 _sample/1.in_ 与 _sample/1.ans_。"
fn handle_sample_file_macro(call: &DotCall, problem: &ProblemConfig) -> Vec<Block> {
    debug!("处理 sample_file 宏: {:?}", call);

    // 解析样本ID
    let sample_id: u32 = match call.arguments.as_deref() {
        Some(args) => match args.parse() {
            Ok(id) => {
                debug!("解析到样本ID: {}", id);
                id
            }
            Err(e) => {
                warn!("无效的样本ID: {} -> {:?}", args, e);
                return vec![Block::Paragraph(vec![Inline::Text(format!(
                    "无效的样本ID: {}",
                    args
                ))])];
            }
        },
        None => {
            warn!("sample_file 宏缺少参数");
            return vec![Block::Paragraph(vec![Inline::Text(
                "sample_file 宏需要参数，例如: sample_file(1)".to_string(),
            )])];
        }
    };

    let problem_name = problem.name.clone();

    info!("生成文件引用: sample_file({})", sample_id);

    // 返回 Markdown 段落，使用 Emphasis 来强调文件名
    vec![Block::Paragraph(vec![
        Inline::Text("见选手目录下的 ".to_string()),
        Inline::Emphasis(vec![Inline::Text(format!(
            "{}/{}{}.in",
            &problem_name, &problem_name, sample_id
        ))]),
        Inline::Text(" 与 ".to_string()),
        Inline::Emphasis(vec![Inline::Text(format!(
            "{}/{}{}.ans",
            &problem_name, &problem_name, sample_id
        ))]),
        Inline::Text("。".to_string()),
    ])]
}

// 完整的 expand_macro 函数
pub fn expand_macro(
    ast: Document,
    path: &PathBuf,
    problem: &ProblemConfig,
) -> Result<Document, Box<dyn std::error::Error>> {
    debug!("开始扩展宏，路径: {:?}", path);

    let path_clone = path.clone();
    let problem_clone = problem.clone();

    let mut transformer = MacroTransformer {
        block_expander: Rc::new(move |content| {
            debug!("处理宏内容: {}", content);

            // 1. 安全解析宏调用
            let call = match nom::parse_dot_call(content) {
                Ok((_, call)) => {
                    debug!("成功解析宏调用: {:?}", call);
                    call
                }
                Err(e) => {
                    warn!("解析宏调用失败: {} -> {:?}", content, e);
                    return vec![Block::Paragraph(vec![Inline::Text(format!(
                        "解析宏调用失败: {}",
                        e
                    ))])];
                }
            };

            // 2. 根据宏类型分发处理
            match call.path.first().map(|s| s.as_str()) {
                Some("sample") => handle_sample_macro(&call, &problem_clone, &path_clone),
                Some("sample_file") => handle_sample_file_macro(&call, &problem_clone),
                _ => {
                    warn!("未知的块宏: {}", call.path.join("."));
                    vec![Block::Paragraph(vec![Inline::Text(format!(
                        "未知的块宏: {}",
                        call.path.join(".")
                    ))])]
                }
            }
        }),
    };

    info!("开始扩展AST文档");
    let doc = ast.expand_with(&mut transformer);

    match doc.first() {
        Some(first_doc) => {
            info!("宏扩展完成");
            Ok(first_doc.clone())
        }
        None => {
            error!("扩展后文档为空");
            Err("扩展后文档为空".into())
        }
    }
}
