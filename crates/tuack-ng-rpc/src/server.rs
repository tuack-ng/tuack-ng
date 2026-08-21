use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::RwLock;

use tuack_config::lang::Language;

use crate::adapter;
use crate::adapter::ren::RenTaskHandle;
use crate::adapter::test::{RunHandle, RunState};
use crate::jsonrpc::{
    INVALID_CONFIG, INVALID_PARAMS, INVALID_PROJECT, INVALID_REQUEST, METHOD_NOT_FOUND, Response,
    RpcError, SESSION_NOT_FOUND,
};
use crate::jsonrpc::{Incoming, Request};
use crate::protocol::{Target, events, path_to_uri, unescape_segment, uri_to_path};
use crate::session::{EventEmitter, RpcContext, assets_dirs, load_config, load_languages};

#[derive(Debug, Clone, Copy, PartialEq)]
enum ServerState {
    Created,
    Initialized,
    Shutdown,
}

pub struct Server {
    state: RwLock<ServerState>,
    sessions: RwLock<HashMap<String, Arc<RpcContext>>>,
    runs: RwLock<HashMap<String, Arc<RunHandle>>>,
    tasks: RwLock<HashMap<String, Arc<RenTaskHandle>>>,
    emitter: Arc<EventEmitter>,
    next_id: AtomicU64,
    exit_flag: Arc<AtomicBool>,
    assets_dirs: Vec<PathBuf>,
    languages: IndexMap<String, Language>,
}

impl Server {
    pub fn new() -> anyhow::Result<Self> {
        let assets_dirs = assets_dirs()?;
        let languages = load_languages(&assets_dirs)?;
        Ok(Server {
            state: RwLock::new(ServerState::Created),
            sessions: RwLock::new(HashMap::new()),
            runs: RwLock::new(HashMap::new()),
            tasks: RwLock::new(HashMap::new()),
            emitter: Arc::new(EventEmitter::new()),
            next_id: AtomicU64::new(1),
            exit_flag: Arc::new(AtomicBool::new(false)),
            assets_dirs,
            languages,
        })
    }

    pub fn exit_flag(&self) -> Arc<AtomicBool> {
        self.exit_flag.clone()
    }

    pub async fn handle(&self, incoming: Incoming) -> Option<Response> {
        match incoming {
            Incoming::Request(req) => {
                if req.jsonrpc != "2.0" {
                    return Some(Response::err(
                        req.id,
                        RpcError::new(INVALID_REQUEST, "jsonrpc 版本必须为 2.0"),
                    ));
                }
                Some(self.dispatch_request(req).await)
            }
            Incoming::Notification(notif) => {
                if notif.jsonrpc == "2.0" && notif.method == "exit" {
                    *self.state.write().await = ServerState::Shutdown;
                    self.exit_flag.store(true, Ordering::SeqCst);
                }
                None
            }
        }
    }

    async fn dispatch_request(&self, req: Request) -> Response {
        let state = *self.state.read().await;
        match req.method.as_str() {
            "initialize" => {
                if state != ServerState::Created {
                    return Response::err(req.id, RpcError::new(INVALID_REQUEST, "服务已初始化"));
                }
            }
            "shutdown" => {
                if state != ServerState::Initialized {
                    return Response::err(req.id, RpcError::new(INVALID_REQUEST, "服务尚未初始化"));
                }
            }
            _ => {
                if state != ServerState::Initialized {
                    return Response::err(
                        req.id,
                        RpcError::new(INVALID_REQUEST, "尚未初始化，请先调用 initialize"),
                    );
                }
            }
        }

        let result = self.dispatch(&req.method, req.params).await;
        match result {
            Ok(value) => Response::ok(req.id, value),
            Err(e) => Response::err(req.id, e),
        }
    }

    async fn dispatch(&self, method: &str, params: Option<Value>) -> Result<Value, RpcError> {
        match method {
            "initialize" => {
                *self.state.write().await = ServerState::Initialized;
                Ok(json!({
                    "protocolVersion": "0.1",
                    "serverInfo": { "name": "tuack-ng-rpc", "version": env!("CARGO_PKG_VERSION") },
                    "capabilities": ["workspace", "config", "problem", "run", "ren"],
                }))
            }
            "shutdown" => {
                *self.state.write().await = ServerState::Shutdown;
                Ok(Value::Null)
            }
            "workspace/open" => self.workspace_open(params).await,
            "workspace/close" => self.workspace_close(params).await,
            "workspace/list" => self.workspace_list().await,
            "config/schema" => Ok(adapter::config::schema()),
            "config/get" => {
                let (ctx, p) = self.session_params(params).await?;
                adapter::config::get(&ctx, p).await
            }
            "config/set" => {
                let (ctx, p) = self.session_params(params).await?;
                adapter::config::set(&ctx, p).await
            }
            "config/reload" => {
                let (ctx, p) = self.session_params(params).await?;
                adapter::config::reload(&ctx, p).await
            }
            "config/migrate" => {
                let (ctx, p) = self.session_params(params).await?;
                adapter::config::migrate(&ctx, p).await
            }
            "problem/list" => {
                let (ctx, p) = self.session_params(params).await?;
                adapter::problem::list(&ctx, p).await
            }
            "problem/get" => {
                let (ctx, p) = self.session_params(params).await?;
                adapter::problem::get(&ctx, p).await
            }
            "run/create" => self.run_create(params).await,
            "run/judge" => self.run_judge(params).await,
            "run/score" => self.run_score(params).await,
            "run/cancel" => self.run_cancel(params).await,
            "run/get" => self.run_get(params).await,
            "ren/preview" => {
                let (ctx, p) = self.session_params(params).await?;
                adapter::ren::preview(&ctx, p).await
            }
            "ren/run" => self.ren_run(params).await,
            "ren/cancel" => self.ren_cancel(params).await,
            "ren/get" => self.ren_get(params).await,
            _ => Err(RpcError::new(
                METHOD_NOT_FOUND,
                format!("方法不存在：{}", method),
            )),
        }
    }

    async fn session_params(
        &self,
        params: Option<Value>,
    ) -> Result<(Arc<RpcContext>, Option<Value>), RpcError> {
        let p = params.unwrap_or_else(|| json!({}));
        let sid = p
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError::new(INVALID_PARAMS, "缺少 sessionId"))?;
        let ctx = self
            .sessions
            .read()
            .await
            .get(sid)
            .cloned()
            .ok_or_else(|| RpcError::new(SESSION_NOT_FOUND, "会话不存在"))?;
        Ok((ctx, Some(p)))
    }

    async fn get_run(&self, session_id: &str, run_id: &str) -> Result<Arc<RunHandle>, RpcError> {
        self.runs
            .read()
            .await
            .get(run_id)
            .filter(|r| r.session_id == session_id)
            .cloned()
            .ok_or_else(|| RpcError::new(crate::jsonrpc::RUN_NOT_FOUND, "run 不存在"))
    }

    // ---- workspace ----

    async fn workspace_open(&self, params: Option<Value>) -> Result<Value, RpcError> {
        #[derive(Deserialize)]
        struct P {
            uri: String,
        }
        let p: P = parse_params(params)?;
        let path = uri_to_path(&p.uri).map_err(|e| RpcError::new(INVALID_PARAMS, e))?;
        let path = dunce::canonicalize(&path)
            .map_err(|e| RpcError::new(INVALID_PARAMS, format!("路径无效：{}", e)))?;
        let config = load_config(&path).map_err(|e| {
            RpcError::new(
                crate::jsonrpc::INTERNAL_ERROR,
                format!("配置加载失败：{}", e),
            )
        })?;

        let id = format!("s-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let ctx = Arc::new(RpcContext {
            id: id.clone(),
            cwd: path.clone(),
            assets_dirs: self.assets_dirs.clone(),
            languages: self.languages.clone(),
            config: RwLock::new(config.clone()),
            revision: AtomicU64::new(0),
            emitter: self.emitter.clone(),
        });
        self.sessions.write().await.insert(id.clone(), ctx);

        let contest = config.as_ref().map(|c| {
            let contest = &c.config;
            let days: Vec<String> = contest.subconfig.keys().cloned().collect();
            json!({
                "name": contest.name,
                "days": days,
                "uri": path_to_uri(&contest.path),
            })
        });
        Ok(json!({
            "sessionId": id,
            "workspace": { "uri": path_to_uri(&path) },
            "contest": contest.unwrap_or(Value::Null),
        }))
    }

    async fn workspace_close(&self, params: Option<Value>) -> Result<Value, RpcError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            session_id: String,
        }
        let p: P = parse_params(params)?;
        let sid = &p.session_id;

        let run_ids: Vec<String> = self
            .runs
            .read()
            .await
            .iter()
            .filter(|(_, r)| &r.session_id == sid)
            .map(|(id, _)| id.clone())
            .collect();
        for rid in run_ids {
            if let Some(run) = self.runs.read().await.get(&rid) {
                let mut state = run.state.lock().unwrap();
                if *state != RunState::Closed {
                    let _ = run.cancel.send(true);
                    *state = RunState::Closed;
                    self.emitter.emit(
                        "run/finished",
                        events::run_finished(sid, &rid, "cancelled", None),
                    );
                }
            }
            self.runs.write().await.remove(&rid);
        }

        let task_ids: Vec<String> = self
            .tasks
            .read()
            .await
            .iter()
            .filter(|(_, t)| &t.session_id == sid)
            .map(|(id, _)| id.clone())
            .collect();
        for tid in task_ids {
            if let Some(task) = self.tasks.read().await.get(&tid) {
                let _ = task.cancel.send(true);
            }
            self.tasks.write().await.remove(&tid);
        }

        self.sessions
            .write()
            .await
            .remove(sid)
            .ok_or_else(|| RpcError::new(SESSION_NOT_FOUND, "会话不存在"))?;
        Ok(Value::Null)
    }

    async fn workspace_list(&self) -> Result<Value, RpcError> {
        let sessions: Vec<Value> = self
            .sessions
            .read()
            .await
            .iter()
            .map(|(id, ctx)| json!({ "sessionId": id, "uri": path_to_uri(&ctx.cwd) }))
            .collect();
        Ok(json!({ "sessions": sessions }))
    }

    // ---- run ----

    async fn run_create(&self, params: Option<Value>) -> Result<Value, RpcError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            session_id: String,
            problem: String,
            target: String,
            tester: Option<String>,
        }
        let p: P = parse_params(params)?;
        let ctx = self
            .sessions
            .read()
            .await
            .get(&p.session_id)
            .cloned()
            .ok_or_else(|| RpcError::new(SESSION_NOT_FOUND, "会话不存在"))?;
        let target = Target::parse(&p.target).map_err(|e| RpcError::new(INVALID_PARAMS, e))?;

        let config = ctx
            .config
            .read()
            .await
            .clone()
            .ok_or_else(|| RpcError::new(INVALID_PROJECT, "没有有效的工程"))?;
        let (day_name, prob_name) = parse_problem_id(&p.problem)?;
        let day_config = config
            .config
            .subconfig
            .get(&day_name)
            .cloned()
            .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("找不到 day: {}", day_name)))?;
        let problem_config = day_config
            .subconfig
            .get(&prob_name)
            .cloned()
            .ok_or_else(|| {
                RpcError::new(INVALID_CONFIG, format!("找不到 problem: {}", prob_name))
            })?;

        let tester = p.tester.clone().unwrap_or_else(|| "std".to_string());
        if let Some(t) = &p.tester {
            if !problem_config.tests.contains_key(t) {
                return Err(RpcError::new(
                    INVALID_CONFIG,
                    format!("找不到测试代码：{}", t),
                ));
            }
        }

        let rid = format!("r-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let session_id = ctx.id.clone();
        let handle = Arc::new(RunHandle::new(
            rid.clone(),
            session_id.clone(),
            p.problem.clone(),
            target,
            tester.clone(),
        ));
        self.runs.write().await.insert(rid.clone(), handle.clone());

        self.emitter.emit(
            "run/started",
            events::run_started(&session_id, &rid, &p.problem, target.as_str(), &tester),
        );

        tokio::spawn(async move {
            adapter::test::prepare(ctx, handle, problem_config, day_config).await;
        });

        Ok(json!({ "runId": rid }))
    }

    async fn run_judge(&self, params: Option<Value>) -> Result<Value, RpcError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            session_id: String,
            run_id: String,
            test_id: String,
        }
        let p: P = parse_params(params)?;
        let ctx = self
            .sessions
            .read()
            .await
            .get(&p.session_id)
            .cloned()
            .ok_or_else(|| RpcError::new(SESSION_NOT_FOUND, "会话不存在"))?;
        let run = self.get_run(&p.session_id, &p.run_id).await?;
        adapter::test::judge(&ctx, run, p.test_id).await
    }

    async fn run_score(&self, params: Option<Value>) -> Result<Value, RpcError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            session_id: String,
            run_id: String,
        }
        let p: P = parse_params(params)?;
        self.require_session(&p.session_id).await?;
        let run = self.get_run(&p.session_id, &p.run_id).await?;
        adapter::test::score(run).await
    }

    async fn run_cancel(&self, params: Option<Value>) -> Result<Value, RpcError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            session_id: String,
            run_id: String,
        }
        let p: P = parse_params(params)?;
        let ctx = self
            .sessions
            .read()
            .await
            .get(&p.session_id)
            .cloned()
            .ok_or_else(|| RpcError::new(SESSION_NOT_FOUND, "会话不存在"))?;
        let run = self.get_run(&p.session_id, &p.run_id).await?;
        adapter::test::cancel(&ctx, run);
        Ok(Value::Null)
    }

    async fn run_get(&self, params: Option<Value>) -> Result<Value, RpcError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            session_id: String,
            run_id: String,
        }
        let p: P = parse_params(params)?;
        self.require_session(&p.session_id).await?;
        let run = self.get_run(&p.session_id, &p.run_id).await?;
        adapter::test::get(run).await
    }

    // ---- ren ----

    async fn ren_run(&self, params: Option<Value>) -> Result<Value, RpcError> {
        let params: adapter::ren::RunParams = parse_params(params)?;
        let ctx = self
            .sessions
            .read()
            .await
            .get(&params.session_id)
            .cloned()
            .ok_or_else(|| RpcError::new(SESSION_NOT_FOUND, "会话不存在"))?;

        let scope = match &params.scope {
            Some(s) => {
                crate::protocol::Scope::parse(s).map_err(|e| RpcError::new(INVALID_PARAMS, e))?
            }
            None => crate::protocol::Scope::Contest,
        };
        let scope_str = params
            .scope
            .clone()
            .unwrap_or_else(|| "contest".to_string());

        // 校验模板存在，失败立即返回
        adapter::ren::validate_manifest(&ctx.assets_dirs, &params.template)?;

        let tid = format!("t-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let handle = Arc::new(RenTaskHandle::new(
            tid.clone(),
            params.session_id.clone(),
            params.template.clone(),
        ));
        self.tasks.write().await.insert(tid.clone(), handle.clone());

        self.emitter.emit(
            "ren/started",
            events::ren_started(&params.session_id, &tid, &params.template, &scope_str),
        );

        tokio::spawn(async move {
            adapter::ren::run_task(ctx, handle, scope).await;
        });

        Ok(json!({ "taskId": tid }))
    }

    async fn ren_cancel(&self, params: Option<Value>) -> Result<Value, RpcError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            session_id: String,
            task_id: String,
        }
        let p: P = parse_params(params)?;
        let ctx = self
            .sessions
            .read()
            .await
            .get(&p.session_id)
            .cloned()
            .ok_or_else(|| RpcError::new(SESSION_NOT_FOUND, "会话不存在"))?;
        let task = self.get_task(&p.session_id, &p.task_id).await?;
        adapter::ren::cancel(&ctx, task);
        Ok(Value::Null)
    }

    async fn ren_get(&self, params: Option<Value>) -> Result<Value, RpcError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            session_id: String,
            task_id: String,
        }
        let p: P = parse_params(params)?;
        self.require_session(&p.session_id).await?;
        let task = self.get_task(&p.session_id, &p.task_id).await?;
        adapter::ren::get(task).await
    }

    async fn get_task(
        &self,
        session_id: &str,
        task_id: &str,
    ) -> Result<Arc<RenTaskHandle>, RpcError> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .filter(|t| t.session_id == session_id)
            .cloned()
            .ok_or_else(|| RpcError::new(crate::jsonrpc::RUN_NOT_FOUND, "渲染任务不存在"))
    }

    async fn require_session(&self, session_id: &str) -> Result<(), RpcError> {
        self.sessions
            .read()
            .await
            .contains_key(session_id)
            .then_some(())
            .ok_or_else(|| RpcError::new(SESSION_NOT_FOUND, "会话不存在"))
    }
}

fn parse_problem_id(id: &str) -> Result<(String, String), RpcError> {
    let parts: Vec<String> = id.split('/').map(unescape_segment).collect();
    match parts.as_slice() {
        [day, problem] if !day.is_empty() && !problem.is_empty() => {
            Ok((day.clone(), problem.clone()))
        }
        _ => Err(RpcError::new(
            INVALID_CONFIG,
            "问题标识应为 <day>/<problem>",
        )),
    }
}

fn parse_params<T: serde::de::DeserializeOwned>(params: Option<Value>) -> Result<T, RpcError> {
    let value = params.unwrap_or_else(|| json!({}));
    serde_json::from_value(value).map_err(|_| RpcError::new(INVALID_PARAMS, "参数非法"))
}
