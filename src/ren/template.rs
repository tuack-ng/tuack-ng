use crate::prelude::*;
use crate::ren::manifest::TemplateManifest;
use crate::ren::tools;
use anyhow::Result;
use colored::Colorize;
use minijinja::{Environment, Value, context};

fn input_file(problem: &ProblemConfig, file_io: bool) -> Result<String, minijinja::Error> {
    Ok(if file_io {
        format!("从文件 _{}.in_ 中读入数据。", problem.name)
    } else {
        "从标准输入读入数据。".to_string()
    })
}

fn output_file(problem: &ProblemConfig, file_io: bool) -> Result<String, minijinja::Error> {
    Ok(if file_io {
        format!("输出到文件 _{}.out_ 中。", problem.name)
    } else {
        "输出到标准输出。".to_string()
    })
}

/// 处理 sample 函数
fn handle_sample(
    sample_id: u32,
    problem: &ProblemConfig,
    base_path: &Path,
) -> Result<String, minijinja::Error> {
    debug!("处理 sample 函数：{}", sample_id);

    // 查找样例
    let sample_item = match problem.samples.iter().find(|s| s.id == sample_id) {
        Some(item) => item,
        None => {
            msg_warn!(
                "在题目 {} 中未找到样例 {}",
                problem.name.magenta(),
                sample_id.to_string().cyan()
            );
            return Err(minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("未找到样例 {}", sample_id),
            ));
        }
    };

    // 构建 Markdown
    let mut md = String::new();

    // 输入部分
    md.push_str(&format!("## 样例 {} 输入\n\n", sample_id));

    let input_file = &sample_item.input_path();

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
                msg_error!("读取输入文件失败：{:?} -> {}", input_path, e);
                return Err(minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    format!("读取输入文件失败 {}", e),
                ));
            }
        }
    } else {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("文件不存在 {}", input_path.display()),
        ));
    }

    // 输出部分（修改这里）
    md.push_str(&format!("## 样例 {} 输出\n\n", sample_id));

    let output_file = &sample_item.output_path();

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
                msg_error!("读取输出文件失败：{:?} -> {}", output_path, e);
                return Err(minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    format!("读取输出文件失败 {}", e),
                ));
            }
        }
    } else {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("输出文件不存在 {}", output_path.display()),
        ));
    }

    debug!("成功生成样例 {} 的 Markdown", sample_id);
    Ok(md)
}

/// 处理 sample_file 函数
fn handle_sample_file(sample_id: u32, problem: &ProblemConfig) -> Result<String, minijinja::Error> {
    debug!("处理 sample_file 函数：{}", sample_id);

    // 检查样例是否存在
    if !problem.samples.iter().any(|s| s.id == sample_id) {
        msg_warn!(
            "在题目 {} 中未找到样例 {}",
            problem.name.magenta(),
            sample_id.to_string().cyan()
        );
    }

    let text = format!(
        "见选手目录下的 _{0}/{0}{1}.in_ 与 _{0}/{0}{1}.ans_。",
        problem.name, sample_id
    );

    debug!("生成文件引用：sample_file({}) -> {}", sample_id, text);
    Ok(text)
}

fn handle_lua_table(
    path: String,
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    contest: &ContestConfig,
) -> Result<String, minijinja::Error> {
    let full_path = problem.path.join(&path);
    super::lua::render_template(&full_path, problem, day, contest).map_err(|e| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("Lua 表格渲染失败：{}", e),
        )
    })
}

/// 使用模板渲染函数
pub fn render_template(
    template: &str,
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    contest: &ContestConfig,
    base_path: PathBuf,
    manifest: TemplateManifest,
) -> Result<String> {
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
                    Ok(tools::hn(num, style))
                }
            }),
        ),
        (
            "comma",
            Value::from_function({
                move |num: i64| -> Result<String, minijinja::Error> { Ok(tools::comma(num)) }
            }),
        ),
        (
            "cases",
            Value::from_function({
                move |value: Value| -> Result<String, minijinja::Error> {
                    let cases_vec: Vec<i32> = if let Some(i) = value.as_i64() {
                        vec![i as i32]
                    } else {
                        value
                            .try_iter()?
                            .map(|v| {
                                v.as_i64().map(|i| i as i32).ok_or_else(|| {
                                    minijinja::Error::new(
                                        minijinja::ErrorKind::InvalidOperation,
                                        "cases filter expects integers",
                                    )
                                })
                            })
                            .collect::<Result<Vec<i32>, _>>()?
                    };
                    Ok(tools::cases(&cases_vec))
                }
            }),
        ),
    ]);

    let statement = HashMap::from([
        (
            "input_file",
            Value::from_function({
                let problem = problem.clone();
                move || -> Result<String, minijinja::Error> {
                    input_file(&problem, problem.file_io.unwrap_or(manifest.file_io))
                }
            }),
        ),
        (
            "output_file",
            Value::from_function({
                let problem = problem.clone();
                move || -> Result<String, minijinja::Error> {
                    output_file(&problem, problem.file_io.unwrap_or(manifest.file_io))
                }
            }),
        ),
        (
            "table",
            Value::from_function({
                let problem = problem.clone();
                let day = day.clone();
                let contest = contest.clone();
                move |path: String| -> Result<String, minijinja::Error> {
                    handle_lua_table(path, &problem, &day, &contest)
                }
            }),
        ),
    ]);

    // 创建上下文
    let ctx = context! {
        problem => problem,
        day => day,
        contest => contest,
        data_cases => problem.orig_data,
        sample_cases => problem.samples,

        sample => sample,
        tools => tools,
        statement => statement,
        s => statement
    };

    // 渲染模板
    let result = env.render_str(template, ctx)?;
    Ok(result)
}
