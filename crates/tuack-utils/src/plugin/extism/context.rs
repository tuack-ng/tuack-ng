//! extism 插件共用的宿主上下文与 host 函数。

use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use extism::convert::Json;
use extism::{CurrentPlugin, UserData, Val};
use path_clean::PathClean;

use tuack_lib::data::Reader;
use tuack_lib::ren::CommandResult;
use tuack_lib::utils::asset::AssetProvider;
use tuack_lib::utils::output::{OutputFile, OutputSpec};

use crate::keepalive::KeepAliveReader;

use crate::prelude::*;

/// 资产流句柄表：宿主与插件上下文共享，call 结束后宿主仍可取回未消费的流。
pub(crate) type AssetStreams = Arc<Mutex<HashMap<u64, Box<dyn Reader>>>>;

/// 插件调用期间的主机上下文（经 `call_with_host_context` 注入，供 host 函数访问）。
pub(crate) struct PluginContext {
    /// 题目资源提供方；处理器无题目资源时为 `None`
    assets: Option<Box<dyn AssetProvider>>,
    streams: AssetStreams,
    next_id: AtomicU64,
    tmp_dir: PathBuf,
    /// 允许执行的宿主可执行文件白名单
    command: Vec<String>,
}

impl PluginContext {
    pub(crate) fn new(
        assets: Option<Box<dyn AssetProvider>>,
        tmp_dir: PathBuf,
        streams: AssetStreams,
        command: Vec<String>,
    ) -> Self {
        Self {
            assets,
            streams,
            next_id: AtomicU64::new(0),
            tmp_dir,
            command,
        }
    }

    /// 打开第 `problem_idx` 题的资产 `url`，返回句柄。
    fn open_asset(&self, problem_idx: u64, url: &str) -> Result<u64> {
        let assets = self.assets.as_ref().context("该插件不支持访问题目资源")?;
        let stream = assets.load(problem_idx, Path::new(url))?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.streams.lock().unwrap().insert(id, stream);
        Ok(id)
    }

    /// 读取资产的一段，读到 EOF 返回空。
    fn read_asset(&self, asset_id: u64, len: u64) -> Result<Vec<u8>> {
        // `len` 由插件传入，钳到上限避免恶意/有 bug 的插件让宿主任意分配。
        const MAX_ASSET_CHUNK: u64 = 1024 * 1024;
        let len = len.min(MAX_ASSET_CHUNK);
        let mut streams = self.streams.lock().unwrap();
        let stream = streams
            .get_mut(&asset_id)
            .ok_or_else(|| anyhow!("无效的资产句柄：{}", asset_id))?;
        let mut buf = vec![0u8; len as usize];
        let n = stream.read(&mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    /// 关闭资产句柄。
    fn close_asset(&self, asset_id: u64) {
        self.streams.lock().unwrap().remove(&asset_id);
    }

    /// 把资产流从当前位置直接拷贝到 `dest` 对应的 WASI 文件（不参与产物回传）。
    fn copy_asset(&self, asset_id: u64, dest: &str) -> Result<()> {
        let host_dest = resolve_within(&self.tmp_dir, dest)?;
        if let Some(parent) = host_dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut streams = self.streams.lock().unwrap();
        let stream = streams
            .get_mut(&asset_id)
            .ok_or_else(|| anyhow!("无效的资产句柄：{}", asset_id))?;
        let mut f = std::fs::File::create(&host_dest)?;
        std::io::copy(stream, &mut f)?;
        Ok(())
    }

    /// 执行宿主命令（受白名单约束）。
    fn run_command(&self, args: &[String], cwd: &str) -> Result<CommandResult> {
        if args.is_empty() {
            bail!("命令参数不能为空");
        }
        let program = &args[0];
        if !self.command.iter().any(|c| c == "*" || c == program) {
            bail!("插件无权执行命令：{}", program);
        }
        let mut cmd = std::process::Command::new(program);
        cmd.args(&args[1..]);
        cmd.current_dir(self.resolve_cwd(cwd)?);
        let output = cmd
            .output()
            .map_err(|e| anyhow!(e).context("执行命令失败"))?;
        Ok(CommandResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    /// 把插件传入的 `cwd` 限定在工作区内：空串为工作区根；`get_path` 返回的宿主路径直接用；
    /// 其余按 WASI 路径解析。越界报错。
    fn resolve_cwd(&self, cwd: &str) -> Result<PathBuf> {
        if cwd.is_empty() {
            return Ok(self.tmp_dir.clone());
        }
        let path = Path::new(cwd);
        if path.is_absolute() && path.clean().starts_with(&self.tmp_dir) {
            return Ok(path.clean());
        }
        resolve_within(&self.tmp_dir, cwd)
    }
}

/// 取当前调用的主机上下文，执行 `f`（隔离 borrow 作用域，结束后才能写返回值）。
fn with_context<T>(
    plugin: &mut CurrentPlugin,
    f: impl FnOnce(&mut PluginContext) -> Result<T>,
) -> Result<T> {
    let ctx = plugin.host_context::<PluginContext>()?;
    f(ctx)
}

fn asset_open(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let problem_idx: u64 = plugin.memory_get_val(&inputs[0])?;
    let url: String = plugin.memory_get_val(&inputs[1])?;
    let id = with_context(plugin, |ctx| ctx.open_asset(problem_idx, &url))?;
    plugin.memory_set_val(&mut outputs[0], id)?;
    Ok(())
}

fn asset_read(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let asset_id: u64 = plugin.memory_get_val(&inputs[0])?;
    let len: u64 = plugin.memory_get_val(&inputs[1])?;
    let buf = with_context(plugin, |ctx| ctx.read_asset(asset_id, len))?;
    plugin.memory_set_val(&mut outputs[0], buf)?;
    Ok(())
}

fn asset_close(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    _outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let asset_id: u64 = plugin.memory_get_val(&inputs[0])?;
    with_context(plugin, |ctx| {
        ctx.close_asset(asset_id);
        Ok(())
    })?;
    Ok(())
}

fn asset_copy(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    _outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let asset_id: u64 = plugin.memory_get_val(&inputs[0])?;
    let dest: String = plugin.memory_get_val(&inputs[1])?;
    with_context(plugin, |ctx| ctx.copy_asset(asset_id, &dest))?;
    Ok(())
}

fn run_command(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let args: Json<Vec<String>> = plugin.memory_get_val(&inputs[0])?;
    let cwd: Json<String> = plugin.memory_get_val(&inputs[1])?;
    let result = with_context(plugin, |ctx| ctx.run_command(&args.0, &cwd.0))?;
    plugin.memory_set_val(&mut outputs[0], Json(result))?;
    Ok(())
}

fn get_path(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let path: String = plugin.memory_get_val(&inputs[0])?;
    let resolved = with_context(plugin, |ctx| resolve_within(&ctx.tmp_dir, &path))?;
    plugin.memory_set_val(&mut outputs[0], resolved.to_string_lossy().to_string())?;
    Ok(())
}

/// 所有插件共用的 host 函数（日志 + 资产 + 命令 + 路径）。
pub(crate) fn common_imports() -> Vec<extism::Function> {
    vec![
        super::log_import(),
        extism::Function::new(
            "asset_open",
            [extism::PTR, extism::PTR],
            [extism::PTR],
            UserData::default(),
            asset_open,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "asset_read",
            [extism::PTR, extism::PTR],
            [extism::PTR],
            UserData::default(),
            asset_read,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "asset_close",
            [extism::PTR],
            [],
            UserData::default(),
            asset_close,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "asset_copy",
            [extism::PTR, extism::PTR],
            [],
            UserData::default(),
            asset_copy,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "run_command",
            [extism::PTR, extism::PTR],
            [extism::PTR],
            UserData::default(),
            run_command,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
        extism::Function::new(
            "host_get_path",
            [extism::PTR],
            [extism::PTR],
            UserData::default(),
            get_path,
        )
        .with_namespace(extism::EXTISM_USER_MODULE),
    ]
}

/// 把 `rel`（WASI 风格，可能带前导 `/`）解析到 `base` 下；规范化后越出 `base` 则报错。
fn resolve_within(base: &Path, rel: &str) -> Result<PathBuf> {
    let stripped = rel.strip_prefix('/').unwrap_or(rel);
    let dest = base.join(stripped).clean();
    if !dest.starts_with(base) {
        bail!("非法路径：{}", rel);
    }
    Ok(dest)
}

/// 校验并规范化产物相对路径（相对基准 `base`）：必须是相对路径，且规范化后不越出 `base`；
/// 返回规范化后的相对路径。
fn normalize_product_path(base: &Path, path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        bail!("非法产物路径（必须为相对路径）：{}", path.display());
    }
    let dest = base.join(path).clean();
    dest.strip_prefix(base)
        .map(Path::to_path_buf)
        .map_err(|_| anyhow!("非法产物路径（越出输出根）：{}", path.display()))
}

/// 把回传的 `OutputSpec` 转成宿主 `OutputFile`：资产流从共享句柄表取出（host-to-host），
/// 文件从插件 WASI 工作区 `/out` 打开；`keepalive` 保证临时目录存活到流被消费。
pub(crate) fn specs_to_outputs(
    specs: Vec<OutputSpec>,
    streams: &AssetStreams,
    out_dir: &Path,
    keepalive: Arc<tempfile::TempDir>,
) -> Result<Vec<OutputFile>> {
    let mut guard = streams.lock().unwrap();
    let mut files = Vec::new();
    for spec in specs {
        match spec {
            OutputSpec::Asset { path, asset_id } => {
                let path = normalize_product_path(out_dir, &path)?;
                let stream = guard
                    .remove(&asset_id)
                    .ok_or_else(|| anyhow!("无效的资产句柄：{}", asset_id))?;
                files.push(OutputFile::File {
                    path,
                    bytes: stream,
                });
            }
            OutputSpec::File { path } => {
                let rel = normalize_product_path(out_dir, &path)?;
                let dest = out_dir.join(&rel);
                let file = std::fs::File::open(&dest)
                    .with_context(|| format!("打开产物文件失败：{}", dest.display()))?;
                files.push(OutputFile::File {
                    path: rel,
                    bytes: Box::new(KeepAliveReader::new(file, keepalive.clone())),
                });
            }
            OutputSpec::Dir(path) => {
                let path = normalize_product_path(out_dir, &path)?;
                files.push(OutputFile::Dir(path));
            }
        }
    }
    Ok(files)
}
