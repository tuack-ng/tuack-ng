use crate::config::ProblemConfig;
use log::{debug, error, info, warn};
use minijinja::{Environment, context};
use std::fs;
use std::path::{Path, PathBuf};

/// 创建专门的问题模板环境
pub fn create_problem_env(problem: &ProblemConfig, base_path: PathBuf) -> Environment<'static> {
    let mut env = Environment::new();

    // sample 函数
    {
        let problem = problem.clone();
        let base_path = base_path.clone();
        env.add_function(
            "sample",
            move |sample_id: u32| -> Result<String, minijinja::Error> {
                handle_sample(sample_id, &problem, &base_path)
            },
        );
    }

    // sample_file
    {
        let problem = problem.clone();
        env.add_function(
            "sample_file",
            move |sample_id: u32| -> Result<String, minijinja::Error> {
                handle_sample_file(sample_id, &problem)
            },
        );
    }

    env
}

/// 处理 sample 函数 - 直接返回Markdown文本
fn handle_sample(
    sample_id: u32,
    problem: &ProblemConfig,
    base_path: &Path,
) -> Result<String, minijinja::Error> {
    debug!("处理 sample 函数: {}", sample_id);

    // 查找样本
    let sample_item = match problem.samples.iter().find(|s| s.id == sample_id) {
        Some(item) => item,
        None => {
            warn!("未找到样本ID: {}", sample_id);
            return Ok(format!("**错误：未找到样本 {}**", sample_id));
        }
    };

    // 构建Markdown
    let mut md = String::new();

    // 输入部分
    md.push_str(&format!("## 样例 {} 输入\n\n", sample_id));

    if let Some(input_file) = &sample_item.input {
        let input_path = base_path.join("sample").join(input_file);
        if input_path.exists() {
            match fs::read_to_string(&input_path) {
                Ok(content) => {
                    md.push_str("```txt\n");
                    md.push_str(&content);
                    if !content.ends_with('\n') {
                        md.push('\n');
                    }
                    md.push_str("```\n\n");
                }
                Err(e) => {
                    error!("读取输入文件失败: {:?} -> {}", input_path, e);
                    md.push_str(&format!("*读取失败：{}*\n\n", e));
                }
            }
        } else {
            md.push_str(&format!("*文件不存在：{}*\n\n", input_path.display()));
        }
    } else {
        md.push_str("*无输入文件*\n\n");
    }

    // 输出部分
    md.push_str(&format!("## 样例 {} 输出\n\n", sample_id));

    if let Some(output_file) = &sample_item.output {
        let output_path = base_path.join("sample").join(output_file);
        if output_path.exists() {
            match fs::read_to_string(&output_path) {
                Ok(content) => {
                    md.push_str("```txt\n");
                    md.push_str(&content);
                    if !content.ends_with('\n') {
                        md.push('\n');
                    }
                    md.push_str("```\n");
                }
                Err(e) => {
                    error!("读取输出文件失败: {:?} -> {}", output_path, e);
                    md.push_str(&format!("*读取失败：{}*", e));
                }
            }
        } else {
            md.push_str(&format!("*文件不存在：{}*", output_path.display()));
        }
    } else {
        md.push_str("*无输出文件*");
    }

    info!("成功生成样例 {} 的Markdown", sample_id);
    Ok(md)
}

/// 处理 sample_file 函数 - 生成文件引用文本
fn handle_sample_file(sample_id: u32, problem: &ProblemConfig) -> Result<String, minijinja::Error> {
    debug!("处理 sample_file 函数: {}", sample_id);

    // 检查样本是否存在
    // if !problem.samples.iter().any(|s| s.id == sample_id) {
    //     warn!("未找到样本ID: {}", sample_id);
    //     return Ok(format!("**错误：未找到样本 {}**", sample_id));
    // }

    // 直接生成Markdown文本
    let text = format!(
        "见选手目录下的 _{0}/{0}{1}.in_ 与 _{0}/{0}{1}.ans_。",
        problem.name, sample_id
    );

    info!("生成文件引用: sample_file({}) -> {}", sample_id, text);
    Ok(text)
}

/// 使用模板渲染函数
pub fn render_template(
    template: &str,
    problem: &ProblemConfig,
    base_path: PathBuf,
) -> Result<String, Box<dyn std::error::Error>> {
    // 创建环境
    let env = create_problem_env(problem, base_path);

    // 创建上下文
    let ctx = context! {
        problem => problem,
    };

    // 渲染模板
    let result = env.render_str(template, ctx)?;
    Ok(result)
}
