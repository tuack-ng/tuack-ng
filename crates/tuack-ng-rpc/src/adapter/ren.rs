use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use tuack_config::lang::Language;
use tuack_config::{ContestConfig, ContestDayConfig, ProblemConfig};
use tuack_lib::ren::{
    DateInfo, Problem, ProblemMeta, RenConfig, RenderDocument, Renderer, SupportLanguage,
};
use tuack_lib::utils::output::OutputFile;
use tuack_ng_parser::parse;
use tuack_utils::assets::FsAssetProvider;
use tuack_utils::ren::manifest::{TargetType, TemplateManifest};
use tuack_utils::ren::markdown::MarkdownRenderer;
use tuack_utils::ren::processors::process_ast;
use tuack_utils::ren::renderers::ImageCollector;
use tuack_utils::ren::template::render_template;
use tuack_utils::ren::typst::TypstRenderer;

use crate::jsonrpc::{INTERNAL_ERROR, INVALID_CONFIG, INVALID_PARAMS, INVALID_PROJECT, RpcError};
use crate::protocol::{Scope, events};
use crate::session::RpcContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenState {
    Running,
    Finished,
    Cancelled,
    Error,
}

impl RenState {
    pub fn as_str(&self) -> &'static str {
        match self {
            RenState::Running => "running",
            RenState::Finished => "finished",
            RenState::Cancelled => "cancelled",
            RenState::Error => "error",
        }
    }
}

/// 渲染任务句柄
pub struct RenTaskHandle {
    pub id: String,
    pub session_id: String,
    pub template: String,
    pub cancel: watch::Sender<bool>,
    pub state: Mutex<RenState>,
    pub progress: Mutex<(u64, u64)>,
    pub tmp_dir: Mutex<Option<PathBuf>>,
    pub files: Mutex<Vec<String>>,
    pub warnings: Mutex<Vec<String>>,
    pub error: Mutex<Option<String>>,
}

impl RenTaskHandle {
    pub fn new(id: String, session_id: String, template: String) -> Self {
        let (cancel, _rx) = watch::channel(false);
        RenTaskHandle {
            id,
            session_id,
            template,
            cancel,
            state: Mutex::new(RenState::Running),
            progress: Mutex::new((0, 0)),
            tmp_dir: Mutex::new(None),
            files: Mutex::new(Vec::new()),
            warnings: Mutex::new(Vec::new()),
            error: Mutex::new(None),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewParams {
    #[allow(dead_code)]
    session_id: String,
    scope: String,
    template: Option<String>,
}

/// 模板展开预览：返回未做 AST 解析/渲染的 Markdown
pub async fn preview(ctx: &RpcContext, params: Option<Value>) -> Result<Value, RpcError> {
    let params: PreviewParams = parse_params(params)?;
    let scope = Scope::parse(&params.scope).map_err(|e| RpcError::new(INVALID_PARAMS, e))?;
    let (day, problem) = match scope {
        Scope::Problem { day, problem } => (day, problem),
        _ => {
            return Err(RpcError::new(
                INVALID_PARAMS,
                "ren/preview 的 scope 必须定位到单个题目（<day>/<problem>）",
            ));
        }
    };

    let config = ctx
        .config
        .read()
        .await
        .clone()
        .ok_or_else(|| RpcError::new(INVALID_PROJECT, "没有有效的工程"))?;
    let contest = &config.config;
    let day_config = contest
        .subconfig
        .get(&day)
        .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("找不到 day: {}", day)))?;
    let problem_config = day_config
        .subconfig
        .get(&problem)
        .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("找不到 problem: {}", problem)))?;

    let manifest = match &params.template {
        Some(name) => load_manifest(&ctx.assets_dirs, name)?,
        None => default_manifest(),
    };

    let statement_path = problem_config.path.join("statement.md");
    if !statement_path.exists() {
        return Err(RpcError::new(
            INVALID_CONFIG,
            format!("未找到题面文件：{}", statement_path.display()),
        ));
    }
    let re = regex::Regex::new(r"<!--[\s\S]*?-->").expect("注释正则");
    let source = std::fs::read_to_string(&statement_path)
        .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("读取题面失败：{}", e)))?;
    // 先在原始文本上逐行插哨兵（行号固定为原始文件行号），再删注释；
    // 注释体的哨兵随注释被移除；注释行的行首哨兵（在 <!-- 标记之前）保留，映射到其留下的空行
    let token = gen_token();
    let sentinel_text = insert_sentinels(&source, &token);
    let content = re.replace_all(&sentinel_text, "");
    let (rendered, warnings) = render_template(
        content.as_ref(),
        problem_config,
        day_config,
        contest,
        problem_config.path.clone(),
        manifest,
    )
    .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("模板展开失败：{}", e)))?;
    let (markdown, line_map) = parse_sentinels(&rendered, &token);

    Ok(json!({ "markdown": markdown, "warnings": warnings, "lineMap": line_map }))
}

/// 生成哨兵 token（纳秒时间戳 hex；仅防误识别，无需加密强度）
fn gen_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos)
}

/// 逐行行首插入哨兵 `[<token>-L<行号>]`（保留空行与尾换行）
fn insert_sentinels(text: &str, token: &str) -> String {
    text.split('\n')
        .enumerate()
        .map(|(i, line)| format!("[{}-L{}]{}", token, i + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 解析渲染结果中的行首哨兵，构建 渲染后行号 -> 来源行号 映射（同一来源行取第一次出现），
/// 移除哨兵后返回纯净 markdown
fn parse_sentinels(rendered: &str, token: &str) -> (String, Vec<Value>) {
    let prefix = format!("[{}-L", token);
    let mut map: BTreeMap<u32, u32> = BTreeMap::new();
    let mut lines_out: Vec<String> = Vec::new();
    for (i, line) in rendered.split('\n').enumerate() {
        let rendered_lineno = (i + 1) as u32;
        if let Some(rest) = line.strip_prefix(&prefix) {
            if let Some(end) = rest.find(']') {
                if let Ok(source) = rest[..end].parse::<u32>() {
                    let content = rest[end + 1..]
                        .strip_prefix(' ')
                        .unwrap_or(&rest[end + 1..]);
                    map.entry(source).or_insert(rendered_lineno);
                    lines_out.push(content.to_string());
                    continue;
                }
            }
        }
        lines_out.push(line.to_string());
    }
    let line_map: Vec<Value> = map
        .iter()
        .map(|(source, rendered)| json!({ "source": source, "rendered": rendered }))
        .collect();
    (lines_out.join("\n"), line_map)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunParams {
    #[allow(dead_code)]
    pub session_id: String,
    pub template: String,
    pub scope: Option<String>,
}

/// 渲染任务：每 day 独立渲染临时目录，产物写入共享输出目录
pub async fn run_task(ctx: Arc<RpcContext>, handle: Arc<RenTaskHandle>, scope: Scope) {
    let manifest = match load_manifest(&ctx.assets_dirs, &handle.template) {
        Ok(m) => m,
        Err(e) => {
            fail(&handle, &ctx, &e.message);
            return;
        }
    };

    let config = match ctx.config.read().await.clone() {
        Some(c) => c,
        None => {
            fail(&handle, &ctx, "没有有效的工程");
            return;
        }
    };
    let contest = config.config.clone();

    let days: Vec<(String, Option<String>)> = match scope {
        Scope::Contest => contest
            .subconfig
            .iter()
            .map(|(name, _)| (name.clone(), None))
            .collect(),
        Scope::Day(day) => vec![(day, None)],
        Scope::Problem { day, problem } => vec![(day, Some(problem))],
    };

    if days.is_empty() {
        fail(&handle, &ctx, "没有可渲染的层级");
        return;
    }

    let output_root = match tempfile::Builder::new().prefix("tuack-ng-ren-").tempdir() {
        Ok(tmp) => tmp,
        Err(e) => {
            fail(&handle, &ctx, &format!("创建临时目录失败：{}", e));
            return;
        }
    };
    // 产物目录交给调用者处理（移动/预览/删除），完成后保留不自动删除
    let output_root_path = output_root.keep();
    *handle.tmp_dir.lock().unwrap() = Some(output_root_path.clone());
    *handle.progress.lock().unwrap() = (0, days.len() as u64);

    let total = days.len() as u64;
    let mut all_files: Vec<String> = Vec::new();
    let mut all_warnings: Vec<String> = Vec::new();

    for (idx, (day_name, problem)) in days.iter().enumerate() {
        if *handle.cancel.borrow() {
            cancelled(&handle, &ctx);
            return;
        }

        let day_config = match contest.subconfig.get(day_name) {
            Some(d) => d.clone(),
            None => {
                fail(&handle, &ctx, &format!("找不到 day: {}", day_name));
                return;
            }
        };

        let (doc, warnings) = match build_render_document(
            &contest,
            &manifest,
            &day_config,
            problem.as_deref(),
            &ctx.languages,
        ) {
            Ok(r) => r,
            Err(e) => {
                fail(&handle, &ctx, &format!("构建渲染文档失败：{}", e));
                return;
            }
        };

        // 每个 day 独立的渲染临时目录（TypstRenderer 需解压模板）
        let day_tmp = match tempfile::Builder::new()
            .prefix("tuack-ng-ren-day-")
            .tempdir()
        {
            Ok(t) => t,
            Err(e) => {
                fail(&handle, &ctx, &format!("创建临时目录失败：{}", e));
                return;
            }
        };

        let renderer: Box<dyn Renderer> = match manifest.target {
            TargetType::Typst => {
                match TypstRenderer::new(day_tmp.path().to_path_buf(), &manifest, &ctx.assets_dirs)
                {
                    Ok(r) => Box::new(r),
                    Err(e) => {
                        ctx.emitter.emit(
                            "ren/output",
                            events::ren_output(
                                &handle.session_id,
                                &handle.id,
                                "renderer",
                                &format!("{:#}", e),
                            ),
                        );
                        fail(&handle, &ctx, "Typst 渲染器初始化失败");
                        return;
                    }
                }
            }
            TargetType::Markdown => Box::new(MarkdownRenderer::new()),
        };

        let (target, files) = match renderer.render(&doc).await {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("{:#}", e);
                ctx.emitter.emit(
                    "ren/output",
                    events::ren_output(&handle.session_id, &handle.id, "renderer", &msg),
                );
                fail(&handle, &ctx, &format!("渲染失败：{}", msg));
                return;
            }
        };

        let file_paths: Vec<String> = files
            .iter()
            .map(|f| match f {
                OutputFile::File { path, .. } | OutputFile::Dir(path) => {
                    path.to_string_lossy().to_string()
                }
            })
            .collect();
        if let Err(e) = write_outputs(&output_root_path, files).await {
            fail(&handle, &ctx, &format!("写入产物失败：{}", e));
            return;
        }

        all_files.push(target.to_string_lossy().to_string());
        all_files.extend(file_paths);
        all_warnings.extend(warnings);
        *handle.progress.lock().unwrap() = ((idx + 1) as u64, total);
        ctx.emitter.emit(
            "ren/progress",
            events::ren_progress(
                &handle.session_id,
                &handle.id,
                (idx + 1) as u64,
                total,
                day_name,
            ),
        );
    }

    *handle.state.lock().unwrap() = RenState::Finished;
    *handle.files.lock().unwrap() = all_files.clone();
    *handle.warnings.lock().unwrap() = all_warnings.clone();
    ctx.emitter.emit(
        "ren/finished",
        events::ren_finished(
            &handle.session_id,
            &handle.id,
            "finished",
            Some(output_root_path.to_string_lossy().to_string()),
            all_files,
            all_warnings,
            None,
        ),
    );
}

pub fn cancel(ctx: &RpcContext, handle: Arc<RenTaskHandle>) {
    let mut state = handle.state.lock().unwrap();
    if *state == RenState::Running {
        let _ = handle.cancel.send(true);
        *state = RenState::Cancelled;
        ctx.emitter.emit(
            "ren/finished",
            events::ren_finished(
                &handle.session_id,
                &handle.id,
                "cancelled",
                None,
                Vec::new(),
                Vec::new(),
                None,
            ),
        );
    }
}

pub async fn get(handle: Arc<RenTaskHandle>) -> Result<Value, RpcError> {
    let state = *handle.state.lock().unwrap();
    let (done, total) = *handle.progress.lock().unwrap();
    let tmp_dir = handle.tmp_dir.lock().unwrap().clone();
    let files = handle.files.lock().unwrap().clone();
    let warnings = handle.warnings.lock().unwrap().clone();
    let error = handle.error.lock().unwrap().clone();

    Ok(json!({
        "state": state.as_str(),
        "template": handle.template,
        "progress": { "done": done, "total": total },
        "tmpDir": tmp_dir.map(|p| Value::String(p.to_string_lossy().to_string())).unwrap_or(Value::Null),
        "files": files.iter().map(|f| json!({ "path": f })).collect::<Vec<_>>(),
        "warnings": warnings,
        "error": error.map(Value::String).unwrap_or(Value::Null),
    }))
}

fn fail(handle: &RenTaskHandle, ctx: &RpcContext, msg: &str) {
    *handle.state.lock().unwrap() = RenState::Error;
    *handle.error.lock().unwrap() = Some(msg.to_string());
    ctx.emitter.emit(
        "ren/finished",
        events::ren_finished(
            &handle.session_id,
            &handle.id,
            "error",
            None,
            Vec::new(),
            Vec::new(),
            Some(msg.to_string()),
        ),
    );
}

fn cancelled(handle: &RenTaskHandle, ctx: &RpcContext) {
    *handle.state.lock().unwrap() = RenState::Cancelled;
    ctx.emitter.emit(
        "ren/finished",
        events::ren_finished(
            &handle.session_id,
            &handle.id,
            "cancelled",
            None,
            Vec::new(),
            Vec::new(),
            None,
        ),
    );
}

fn load_manifest(assets_dirs: &[PathBuf], name: &str) -> Result<TemplateManifest, RpcError> {
    let file = assets_dirs
        .iter()
        .find_map(|d| {
            let p = d.join("templates").join(format!("{}.json", name));
            (p.exists() && p.is_file()).then_some(p)
        })
        .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("没有找到模板 {}", name)))?;
    let content = std::fs::read_to_string(&file)
        .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("读取模板清单失败：{}", e)))?;
    serde_json::from_str(&content)
        .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("解析模板清单失败：{}", e)))
}

/// 校验模板清单存在（供 ren/run 同步校验）
pub fn validate_manifest(assets_dirs: &[PathBuf], name: &str) -> Result<(), RpcError> {
    let exists = assets_dirs.iter().any(|d| {
        let p = d.join("templates").join(format!("{}.json", name));
        p.exists() && p.is_file()
    });
    if exists {
        Ok(())
    } else {
        Err(RpcError::new(
            INVALID_CONFIG,
            format!("没有找到模板 {}", name),
        ))
    }
}

fn default_manifest() -> TemplateManifest {
    TemplateManifest {
        use_pretest: false,
        noi_style: true,
        file_io: true,
        target: TargetType::Markdown,
        filelist: IndexMap::new(),
        processor: Vec::new(),
    }
}

fn build_ren_config(
    config: &ContestConfig,
    day_config: &ContestDayConfig,
    manifest: &TemplateManifest,
    languages: &IndexMap<String, Language>,
) -> anyhow::Result<RenConfig> {
    let date = if let (Some(start), Some(end)) = (day_config.start_time, day_config.end_time) {
        Some(DateInfo { start, end })
    } else {
        None
    };

    let use_pretest = day_config
        .use_pretest
        .or(config.use_pretest)
        .unwrap_or(manifest.use_pretest);
    let noi_style = day_config
        .noi_style
        .or(config.noi_style)
        .unwrap_or(manifest.noi_style);
    let file_io = day_config
        .file_io
        .or(config.file_io)
        .unwrap_or(manifest.file_io);

    let mut support_languages = Vec::new();
    for (lang_key, compile_options) in &day_config.compile {
        let language_name = languages
            .get(lang_key)
            .map(|lang| lang.language.clone())
            .ok_or_else(|| anyhow::anyhow!("在语言配置中未找到 {}", lang_key))?;
        support_languages.push(SupportLanguage {
            name: language_name,
            compile_options: compile_options.clone(),
        });
    }

    if day_config.name.is_empty() {
        anyhow::bail!("比赛日 name 不能为空");
    }

    Ok(RenConfig {
        title: config.title.clone(),
        short_title: config.short_title.clone(),
        day_key: day_config.name.clone(),
        dayname: day_config.title.clone(),
        date,
        use_pretest,
        noi_style,
        file_io,
        support_languages,
    })
}

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
            tuack_config::ProblemType::Program => tuack_lib::ren::ProblemType::Program,
            tuack_config::ProblemType::Output => tuack_lib::ren::ProblemType::Output,
            tuack_config::ProblemType::Interactive => tuack_lib::ren::ProblemType::Interactive,
        },
        time_limit: Duration::from_secs_f64(problem.time_limit),
        memory_limit: problem.memory_limit,
        testcase: problem.runtime.data.len(),
        point_equal,
        submit_filename: submit_filenames,
    }
}

/// 构造一天的渲染文档：读题面 -> 模板展开 -> 解析 -> 处理器 -> 图片扫描登记。
/// 返回渲染文档与所有题目的模板展开警告。
fn build_render_document(
    config: &ContestConfig,
    manifest: &TemplateManifest,
    day_config: &ContestDayConfig,
    problem: Option<&str>,
    languages: &IndexMap<String, Language>,
) -> anyhow::Result<(RenderDocument, Vec<String>)> {
    let problems_to_render: IndexMap<String, &ProblemConfig> = match problem {
        Some(key) => day_config
            .subconfig
            .get(key)
            .map(|c| IndexMap::from([(key.to_string(), c)]))
            .ok_or_else(|| anyhow::anyhow!("未找到问题：{}", key))?,
        None => day_config
            .subconfig
            .iter()
            .map(|(k, v)| (k.clone(), v))
            .collect(),
    };

    let re = regex::Regex::new(r"<!--[\s\S]*?-->").expect("注释正则");
    let mut assets = FsAssetProvider::new();
    let mut problems = Vec::new();
    let mut all_warnings = Vec::new();

    for (idx, (_problem_key, problem_config)) in problems_to_render.iter().enumerate() {
        let statement_path = problem_config.path.join("statement.md");
        if !statement_path.exists() {
            anyhow::bail!("未找到题面文件：{}", statement_path.display());
        }

        let source = std::fs::read_to_string(&statement_path)?;
        let content = re.replace_all(&source, "");
        let (content, warnings) = render_template(
            content.as_ref(),
            problem_config,
            day_config,
            config,
            problem_config.path.clone(),
            manifest.clone(),
        )
        .with_context(|| format!("读取题面文件/展开模板失败：{}", statement_path.display()))?;

        all_warnings.extend(warnings);

        let mut ast = parse(&content);
        ast = process_ast(&mut ast, &manifest.processor)?;

        assets.register(idx as u64, problem_config.path.clone());

        problems.push(Problem {
            idx: idx as u64,
            meta: build_problem_meta(problem_config, day_config),
            ast,
        });
    }

    let precaution_ast = {
        let precaution_path = config.path.join("precaution.md");
        if precaution_path.exists() {
            let ast = parse(&std::fs::read_to_string(&precaution_path)?);
            if !ImageCollector::collect(&ast).is_empty() {
                anyhow::bail!("注意事项不支持图片");
            }
            Some(ast)
        } else {
            None
        }
    };

    let ren_config = build_ren_config(config, day_config, manifest, languages)?;

    Ok((
        RenderDocument {
            config: ren_config,
            problems,
            precaution: precaution_ast,
            assets: Box::new(assets),
        },
        all_warnings,
    ))
}

/// 将产物写入输出目录（文件流式落盘，空目录直接创建）
async fn write_outputs(base: &Path, files: Vec<OutputFile>) -> anyhow::Result<()> {
    for file in files {
        match file {
            OutputFile::Dir(path) => {
                tokio::fs::create_dir_all(base.join(&path)).await?;
            }
            OutputFile::File { path, mut bytes } => {
                let target = base.join(&path);
                if let Some(parent) = target.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                let mut out = tokio::fs::File::create(&target).await?;
                tokio::io::copy(&mut bytes, &mut out).await?;
            }
        }
    }
    Ok(())
}

fn parse_params<T: serde::de::DeserializeOwned>(params: Option<Value>) -> Result<T, RpcError> {
    let value = params.unwrap_or_else(|| Value::Object(Default::default()));
    serde_json::from_value(value).map_err(|_| RpcError::new(INVALID_PARAMS, "参数非法"))
}
