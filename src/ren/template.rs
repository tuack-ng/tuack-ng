use crate::config::{ContestConfig, ContestDayConfig, ProblemConfig};
use log::{debug, error, info, warn};
use minijinja::Value;
use minijinja::{Environment, context};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn input_file(problem: &ProblemConfig) -> Result<String, minijinja::Error> {
    Ok(format!("从文件 _{}.in_ 中读入数据。", problem.name))
}

fn output_file(problem: &ProblemConfig) -> Result<String, minijinja::Error> {
    Ok(format!("输出到文件 _{}.out_ 中。", problem.name))
}

/// 处理 sample 函数
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

/// 处理 sample_file 函数
fn handle_sample_file(sample_id: u32, problem: &ProblemConfig) -> Result<String, minijinja::Error> {
    debug!("处理 sample_file 函数: {}", sample_id);

    // 检查样本是否存在
    if !problem.samples.iter().any(|s| s.id == sample_id) {
        warn!("未找到样本ID: {}", sample_id);
    }

    // 直接生成Markdown文本
    let text = format!(
        "见选手目录下的 _{0}/{0}{1}.in_ 与 _{0}/{0}{1}.ans_。",
        problem.name, sample_id
    );

    info!("生成文件引用: sample_file({}) -> {}", sample_id, text);
    Ok(text)
}

/// 计算一个数的以10为底的对数的整数部分
///
/// # 参数
/// - `num`: f64 - 要计算的数字
///
/// # 返回值
/// - f64 - 对数的整数部分，特殊情况：
///   - 0 -> -inf
///   - 负数 -> NaN
pub fn int_lg(num: f64) -> f64 {
    if num == 0.0 {
        return f64::NEG_INFINITY;
    }
    if num < 0.0 {
        return f64::NAN;
    }

    let mut n = 0;
    let mut temp = num;

    if temp >= 10.0 {
        while temp >= 10.0 {
            temp /= 10.0;
            n += 1;
        }
    } else if temp < 1.0 {
        while temp < 1.0 {
            temp *= 10.0;
            n -= 1;
        }
    }

    n as f64
}

/// 将整数格式化为带有逗号分隔的字符串
///
/// ## 参数
/// - `num`: i64 - 要格式化的整数
///
/// ## 返回值
/// - String - 格式化后的字符串
pub fn comma(num: i64) -> Result<String, minijinja::Error> {
    if num < 0 {
        return Ok(format!("-{}", comma(-num)?));
    }

    let num_str = num.to_string();
    let len = num_str.len();

    if len <= 3 {
        return Ok(num_str);
    }

    let mut result = String::new();
    let mut count = 0;

    for ch in num_str.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
        count += 1;
    }

    Ok(result.chars().rev().collect())
}

/// 格式化数字为适合阅读的表示形式
///
/// ## 参数
/// - `num`: f64 - 要格式化的数字
/// - `style`: Option<&str> - 格式化样式：
///   - Some("x") - 科学记数法
///   - Some(",") - 逗号分隔形式
///   - None - 自动选择最紧凑的形式
pub fn hn(num: f64, style: Option<&str>) -> Result<String, minijinja::Error> {
    if num == 0.0 {
        return Ok("0".to_string());
    }

    let (neg, abs_num) = if num < 0.0 { ("-", -num) } else { ("", num) };

    // 处理整数情况
    if abs_num.fract() == 0.0 {
        let int_num = abs_num as i64;
        return match style {
            Some("x") => {
                let n = int_lg(abs_num) as i32;
                if int_num == 10_i64.pow(n as u32) {
                    Ok(format!("{}10^{{{}}}", neg, n))
                } else {
                    Ok(format!(
                        "{}{} \\times 10^{{{}}}",
                        neg,
                        int_num / 10_i64.pow(n as u32),
                        n
                    ))
                }
            }
            Some(",") => Ok(format!("{}{}", neg, comma(int_num)?)),
            _ => {
                let n = int_lg(abs_num) as i32;
                let scientific_len = if int_num == 10_i64.pow(n as u32) {
                    3 + n.to_string().len() // 10^{n}
                } else {
                    let coeff = int_num / 10_i64.pow(n as u32);
                    coeff.to_string().len() + 8 + n.to_string().len() // coeff \times 10^{n}
                };

                let comma_str = comma(int_num)?;
                let comma_len = comma_str.len() * 4 / 3; // 近似长度计算

                if comma_len <= scientific_len {
                    Ok(format!("{}{}", neg, comma_str))
                } else {
                    if int_num == 10_i64.pow(n as u32) {
                        Ok(format!("{}10^{{{}}}", neg, n))
                    } else {
                        Ok(format!(
                            "{}{} \\times 10^{{{}}}",
                            neg,
                            int_num / 10_i64.pow(n as u32),
                            n
                        ))
                    }
                }
            }
        };
    }

    // 处理浮点数情况
    match style {
        Some("x") | None => {
            let n = int_lg(abs_num) as i32;
            let coeff = abs_num / 10_f64.powi(n);
            Ok(format!("{}{} \\times 10^{{{}}}", neg, coeff, n))
        }
        Some(",") => Ok(format!("{}{}", neg, abs_num)),
        _ => Ok(format!("{}{}", neg, abs_num)),
    }
}

/// 将数字范围转换为紧凑的表示形式
///
/// ## 参数
/// - `cases`: Vec<i32> - 有序的数字列表
///
/// ## 返回值
/// - String - 格式化后的范围字符串，以$开头和结尾
pub fn cases(cases_vec: Vec<i32>) -> Result<String, minijinja::Error> {
    if cases_vec.is_empty() {
        return Ok("$".to_string());
    }

    let mut result = Vec::new();
    let mut start = cases_vec[0];
    let mut end = cases_vec[0];

    for &num in &cases_vec[1..] {
        if num == end + 1 {
            end = num;
        } else {
            if start == end {
                result.push(format!("{}", start));
            } else if start + 1 == end {
                result.push(format!("{}", start));
                result.push(format!("{}", end));
            } else {
                result.push(format!("{} \\sim {}", start, end));
            }
            start = num;
            end = num;
        }
    }

    // 处理最后一段
    if start == end {
        result.push(format!("{}", start));
    } else if start + 1 == end {
        result.push(format!("{}", start));
        result.push(format!("{}", end));
    } else {
        result.push(format!("{} \\sim {}", start, end));
    }

    Ok(format!("${}$", result.join(",")))
}

/// 使用模板渲染函数
pub fn render_template(
    template: &str,
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    contest: &ContestConfig,
    base_path: PathBuf,
) -> Result<String, Box<dyn std::error::Error>> {
    // 创建环境
    let env = Environment::new();

    let sample = HashMap::from([
        (
            "text",
            Value::from_function({
                let problem = problem.clone();
                let base_path = base_path.clone();
                move |sample_id: u32| -> Result<String, minijinja::Error> {
                    handle_sample(sample_id, &problem, &base_path)
                }
            }),
        ),
        (
            "file",
            Value::from_function({
                let problem = problem.clone();
                move |sample_id: u32| -> Result<String, minijinja::Error> {
                    handle_sample_file(sample_id, &problem)
                }
            }),
        ),
    ]);

    let tools = HashMap::from([
        (
            "hn",
            Value::from_function({
                move |num: f64, style: Option<&str>| -> Result<String, minijinja::Error> {
                    hn(num, style)
                }
            }),
        ),
        (
            "comma",
            Value::from_function({
                move |num: i64| -> Result<String, minijinja::Error> { comma(num) }
            }),
        ),
        (
            "cases",
            Value::from_function({
                move |cases_vec: Vec<i32>| -> Result<String, minijinja::Error> { cases(cases_vec) }
            }),
        ),
    ]);

    let statement = HashMap::from([
        (
            "input_file",
            Value::from_function({
                let problem = problem.clone();
                move || -> Result<String, minijinja::Error> { input_file(&problem) }
            }),
        ),
        (
            "output_file",
            Value::from_function({
                let problem = problem.clone();
                move || -> Result<String, minijinja::Error> { output_file(&problem) }
            }),
        ),
    ]);

    // 创建上下文
    let ctx = context! {
        problem => problem,
        day => day,
        contest => contest,
        sample => sample,
        tools => tools,
        statement => statement,
        s => statement
    };

    // 渲染模板
    let result = env.render_str(template, ctx)?;
    Ok(result)
}
