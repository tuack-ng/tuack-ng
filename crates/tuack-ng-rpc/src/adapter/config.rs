use serde::Deserialize;
use serde_json::{Value, json};
use serde_many::AsSerde;
use std::sync::atomic::Ordering;

use tuack_config::{
    CONFIG_FILE_NAME, Config, ContestConfig, ContestDayConfig, FileView, ProblemConfig, save_config,
};

use crate::jsonrpc::{
    INTERNAL_ERROR, INVALID_CONFIG, INVALID_PARAMS, INVALID_PROJECT, REVISION_CONFLICT, RpcError,
};
use crate::protocol::{Scope, path_to_uri};
use crate::session::{RpcContext, load_config};

/// 配置作用域目标（含对象克隆与定位信息）
enum ScopeTarget {
    Contest {
        config: ContestConfig,
    },
    Day {
        day: String,
        config: ContestDayConfig,
    },
    Problem {
        day: String,
        problem: String,
        config: ProblemConfig,
    },
}

impl ScopeTarget {
    fn rel_path(&self) -> String {
        match self {
            ScopeTarget::Contest { .. } => CONFIG_FILE_NAME.to_string(),
            ScopeTarget::Day { day, .. } => format!("{}/{}", day, CONFIG_FILE_NAME),
            ScopeTarget::Problem { day, problem, .. } => {
                format!("{}/{}/{}", day, problem, CONFIG_FILE_NAME)
            }
        }
    }
}

fn resolve(config: &Config, scope: &Scope) -> Result<ScopeTarget, RpcError> {
    let contest = &config.config;
    match scope {
        Scope::Contest => Ok(ScopeTarget::Contest {
            config: contest.clone(),
        }),
        Scope::Day(day) => {
            let day_config = contest
                .subconfig
                .get(day)
                .cloned()
                .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("找不到 day: {}", day)))?;
            Ok(ScopeTarget::Day {
                day: day.clone(),
                config: day_config,
            })
        }
        Scope::Problem { day, problem } => {
            let day_config = contest
                .subconfig
                .get(day)
                .cloned()
                .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("找不到 day: {}", day)))?;
            let problem_config = day_config.subconfig.get(problem).cloned().ok_or_else(|| {
                RpcError::new(INVALID_CONFIG, format!("找不到 problem: {}", problem))
            })?;
            Ok(ScopeTarget::Problem {
                day: day.clone(),
                problem: problem.clone(),
                config: problem_config,
            })
        }
    }
}

fn to_fileview(target: &ScopeTarget) -> Result<Value, RpcError> {
    let v = match target {
        ScopeTarget::Contest { config } => {
            serde_json::to_value(AsSerde::<ContestConfig, FileView>::new(config.clone()))
        }
        ScopeTarget::Day { config, .. } => {
            serde_json::to_value(AsSerde::<ContestDayConfig, FileView>::new(config.clone()))
        }
        ScopeTarget::Problem { config, .. } => {
            serde_json::to_value(AsSerde::<ProblemConfig, FileView>::new(config.clone()))
        }
    };
    v.map_err(|_| RpcError::new(INTERNAL_ERROR, "配置序列化失败"))
}

/// 应用 JSON Pointer（RFC 6901）到文档；`""` 替换整个文档
fn apply_pointer(doc: &mut Value, pointer: &str, value: Value) -> Result<(), String> {
    if pointer.is_empty() {
        *doc = value;
        return Ok(());
    }
    if !pointer.starts_with('/') {
        return Err(format!("无效的 JSON Pointer: {}", pointer));
    }
    let tokens: Vec<String> = pointer[1..]
        .split('/')
        .map(|t| t.replace("~1", "/").replace("~0", "~"))
        .collect();
    let mut cur = doc;
    let last = tokens.len() - 1;
    for (i, tok) in tokens.iter().enumerate() {
        let is_last = i == last;
        match cur {
            Value::Object(map) => {
                if is_last {
                    map.insert(tok.clone(), value);
                    return Ok(());
                }
                cur = map
                    .get_mut(tok)
                    .ok_or_else(|| format!("路径不存在: /{}", tok))?;
            }
            Value::Array(arr) => {
                let idx: usize = tok
                    .parse()
                    .map_err(|_| format!("无效的数组索引: {}", tok))?;
                if is_last {
                    let slot = arr
                        .get_mut(idx)
                        .ok_or_else(|| format!("数组索引越界: {}", idx))?;
                    *slot = value;
                    return Ok(());
                }
                cur = arr
                    .get_mut(idx)
                    .ok_or_else(|| format!("数组索引越界: {}", idx))?;
            }
            _ => return Err(format!("路径存在但不可导航: /{}", tok)),
        }
    }
    unreachable!()
}

fn scope_or_contest(scope: Option<String>) -> Result<Scope, RpcError> {
    match scope {
        Some(s) => Scope::parse(&s).map_err(|e| RpcError::new(INVALID_PARAMS, e)),
        None => Ok(Scope::Contest),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetParams {
    #[allow(dead_code)]
    session_id: String,
    scope: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetParams {
    #[allow(dead_code)]
    session_id: String,
    scope: String,
    field: String,
    value: Value,
    revision: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReloadParams {
    #[allow(dead_code)]
    session_id: String,
    scope: Option<String>,
}

pub fn schema() -> Value {
    super::schema::schema()
}

pub async fn get(ctx: &RpcContext, params: Option<Value>) -> Result<Value, RpcError> {
    let params: GetParams = parse_params(params)?;
    let scope = scope_or_contest(params.scope)?;
    let config = read_config(&ctx).await?;
    let target = resolve(&config, &scope)?;
    let fileview = to_fileview(&target)?;
    let path = target.rel_path();
    let uri = path_to_uri(&config.config.path.join(&path));
    let revision = ctx.revision.load(Ordering::SeqCst);
    Ok(json!({ "revision": revision, "config": fileview, "path": path, "uri": uri }))
}

/// 将修改后的 FileView JSON 反序列化回运行时配置，保留运行时字段
/// （subconfig/path 等 `#[serde(file(skip))]` 字段），再序列化为写回内容
fn serialize_fileview(target: &ScopeTarget, json: Value) -> Result<String, RpcError> {
    match target {
        ScopeTarget::Contest { config: original } => {
            let mut c: ContestConfig =
                serde_json::from_value::<AsSerde<ContestConfig, FileView>>(json)
                    .map_err(|e| RpcError::new(INVALID_CONFIG, format!("配置值校验失败：{}", e)))?
                    .into_inner();
            c.subconfig = original.subconfig.clone();
            c.path = original.path.clone();
            c.save()
                .map_err(|e| RpcError::new(INVALID_CONFIG, format!("配置序列化失败：{}", e)))
        }
        ScopeTarget::Day {
            config: original, ..
        } => {
            let mut c: ContestDayConfig =
                serde_json::from_value::<AsSerde<ContestDayConfig, FileView>>(json)
                    .map_err(|e| RpcError::new(INVALID_CONFIG, format!("配置值校验失败：{}", e)))?
                    .into_inner();
            c.subconfig = original.subconfig.clone();
            c.path = original.path.clone();
            c.save()
                .map_err(|e| RpcError::new(INVALID_CONFIG, format!("配置序列化失败：{}", e)))
        }
        ScopeTarget::Problem {
            config: original, ..
        } => {
            let mut c: ProblemConfig =
                serde_json::from_value::<AsSerde<ProblemConfig, FileView>>(json)
                    .map_err(|e| RpcError::new(INVALID_CONFIG, format!("配置值校验失败：{}", e)))?
                    .into_inner();
            c.use_pretest = original.use_pretest;
            c.noi_style = original.noi_style;
            c.file_io = original.file_io;
            c.path = original.path.clone();
            c.save()
                .map_err(|e| RpcError::new(INVALID_CONFIG, format!("配置序列化失败：{}", e)))
        }
    }
}

pub async fn set(ctx: &RpcContext, params: Option<Value>) -> Result<Value, RpcError> {
    let params: SetParams = parse_params(params)?;
    let scope = Scope::parse(&params.scope).map_err(|e| RpcError::new(INVALID_PARAMS, e))?;

    let mut config_guard = ctx.config.write().await;
    let config = config_guard
        .as_ref()
        .ok_or_else(|| RpcError::new(INVALID_PROJECT, "没有有效的工程"))?;

    if let Some(expected) = params.revision {
        let current = ctx.revision.load(Ordering::SeqCst);
        if expected != current {
            return Err(RpcError::new(
                REVISION_CONFLICT,
                format!("配置 revision 冲突：期望 {}，当前 {}", expected, current),
            ));
        }
    }

    let target = resolve(config, &scope)?;
    let mut fileview = to_fileview(&target)?;
    apply_pointer(&mut fileview, &params.field, params.value)
        .map_err(|e| RpcError::new(INVALID_CONFIG, e))?;
    let content = serialize_fileview(&target, fileview)?;

    let contest_root = config.config.path.clone();
    let rel = target.rel_path();

    // 写回文件
    let write_path = contest_root.join(&rel);
    std::fs::write(&write_path, content)
        .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("写入配置失败：{}", e)))?;

    // 全量重载刷新 session 缓存（保证 runtime 展开一致）
    let new_config = reload_session(&ctx).await;
    *config_guard = new_config;

    let revision = ctx.revision.fetch_add(1, Ordering::SeqCst) + 1;

    // 用重载后的配置重新返回 FileView
    let fresh_config = config_guard
        .as_ref()
        .ok_or_else(|| RpcError::new(INVALID_PROJECT, "没有有效的工程"))?;
    let target = resolve(fresh_config, &scope)?;
    let fileview = to_fileview(&target)?;
    Ok(json!({ "revision": revision, "config": fileview }))
}

pub async fn reload(ctx: &RpcContext, params: Option<Value>) -> Result<Value, RpcError> {
    let params: ReloadParams = parse_params(params)?;
    let scope = scope_or_contest(params.scope)?;

    let mut config_guard = ctx.config.write().await;
    let before = config_guard
        .as_ref()
        .map(|config| resolve(config, &scope))
        .transpose()?
        .map(|target| to_fileview(&target))
        .transpose()?;

    let new_config = reload_session(&ctx).await;
    *config_guard = new_config;

    let config = config_guard
        .as_ref()
        .ok_or_else(|| RpcError::new(INVALID_PROJECT, "没有有效的工程"))?;
    let target = resolve(config, &scope)?;
    let fileview = to_fileview(&target)?;

    let changed = before.as_ref() != Some(&fileview);
    if changed {
        ctx.revision.fetch_add(1, Ordering::SeqCst);
    }
    let revision = ctx.revision.load(Ordering::SeqCst);

    let path = target.rel_path();
    let uri = path_to_uri(&config.config.path.join(&path));
    Ok(json!({ "revision": revision, "config": fileview, "path": path, "uri": uri }))
}

pub async fn migrate(ctx: &RpcContext, _params: Option<Value>) -> Result<Value, RpcError> {
    use tuack_config::msgs::LoadContext;

    let mut loadctx = LoadContext::new_force_migrate();
    let loaded = tuack_config::load_config(&mut loadctx, &ctx.cwd)
        .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("配置迁移失败：{}", e)))?;

    let config = loaded.ok_or_else(|| RpcError::new(INVALID_PROJECT, "没有有效的工程"))?;
    save_config(&config.config, &config.config.path)
        .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("配置写回失败：{}", e)))?;

    *ctx.config.write().await = Some(config);
    ctx.revision.fetch_add(1, Ordering::SeqCst);

    let notices: Vec<String> = loadctx
        .migrated_notices
        .iter()
        .map(|(from, msg)| {
            if msg.is_empty() {
                format!("来自 {} 版本迁移的信息", from + 1)
            } else {
                format!("来自 {} 版本迁移的信息：{}", from + 1, msg)
            }
        })
        .collect();

    Ok(json!({ "migrated": true, "notices": notices }))
}

async fn read_config(ctx: &RpcContext) -> Result<Config, RpcError> {
    ctx.config
        .read()
        .await
        .clone()
        .ok_or_else(|| RpcError::new(INVALID_PROJECT, "没有有效的工程"))
}

async fn reload_session(ctx: &RpcContext) -> Option<Config> {
    load_config(&ctx.cwd).ok().flatten()
}

fn parse_params<T: serde::de::DeserializeOwned>(params: Option<Value>) -> Result<T, RpcError> {
    let value = params.unwrap_or_else(|| Value::Object(Default::default()));
    serde_json::from_value(value).map_err(|_| RpcError::new(INVALID_PARAMS, "参数非法"))
}
