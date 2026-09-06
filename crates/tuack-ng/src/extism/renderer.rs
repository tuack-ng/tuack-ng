//! extism 渲染器插件：宿主侧封装。

use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use extism::convert::Json;
use extism::{CurrentPlugin, Manifest, UserData, Val, Wasm, WasmInput};
use tuack_lib::data::Reader;
use tuack_lib::ren::{CommandResult, RenderDocument, Renderer, RendererOutput};
use tuack_lib::utils::asset::AssetProvider;
use tuack_lib::utils::output::OutputFile;

use crate::prelude::*;

/// 渲染期间的主机上下文（经 `call_with_host_context` 注入，供 host 函数访问）。
struct RenderContext {
    assets: Box<dyn AssetProvider>,
    streams: Mutex<HashMap<u64, Box<dyn Reader>>>,
    next_id: AtomicU64,
    tmp_dir: PathBuf,
}

impl RenderContext {
    /// 打开第 `problem_idx` 题的资产 `url`，返回句柄。
    fn open_asset(&self, problem_idx: u64, url: &str) -> Result<u64> {
        let stream = self.assets.load(problem_idx, Path::new(url))?;
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
}

/// 取当前调用的主机上下文，执行 `f`（隔离 borrow 作用域，结束后才能写返回值）。
fn with_context<T>(
    plugin: &mut CurrentPlugin,
    f: impl FnOnce(&mut RenderContext) -> Result<T>,
) -> Result<T> {
    let ctx = plugin.host_context::<RenderContext>()?;
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

extism::host_fn!(run_command(args: Json<Vec<String>>, cwd: Json<String>) {
    let args = args.0;
    let cwd = cwd.0;
    if args.is_empty() {
        bail!("命令参数不能为空");
    }
    let mut cmd = std::process::Command::new(&args[0]);
    cmd.args(&args[1..]);
    if !cwd.is_empty() {
        cmd.current_dir(&cwd);
    }
    let output = cmd
        .output()
        .map_err(|e| anyhow!(e).context("执行命令失败"))?;
    Ok(Json(CommandResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
        stderr: output.stderr,
    }))
});

fn get_path(
    plugin: &mut CurrentPlugin,
    inputs: &[Val],
    outputs: &mut [Val],
    _user_data: UserData<()>,
) -> Result<(), extism::Error> {
    let path: String = plugin.memory_get_val(&inputs[0])?;
    let resolved = with_context(plugin, |ctx| {
        let rel = path.strip_prefix('/').unwrap_or(&path);
        if Path::new(rel).components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        }) {
            bail!("非法路径：{}", path);
        }
        let resolved = if rel.is_empty() {
            ctx.tmp_dir.clone()
        } else {
            ctx.tmp_dir.join(rel)
        };
        Ok(resolved.to_string_lossy().to_string())
    })?;
    plugin.memory_set_val(&mut outputs[0], resolved)?;
    Ok(())
}

fn renderer_imports() -> Vec<extism::Function> {
    vec![
        crate::extism::log_import(),
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

/// 一个基于 extism 的渲染器插件。
///
/// 插件开启 WASI，宿主把临时目录映射为插件内的 `/tmp`（工作）与
/// `/out`（产物）；插件写文件到 `/out`，宿主扫描后转成产物文件列表。
pub struct ExtismRenderer {
    plugin: Mutex<extism::Plugin>,
    function: String,
    out_dir: PathBuf,
    tmp_dir: PathBuf,
}

impl ExtismRenderer {
    pub fn new(wasm: Vec<u8>, function: String, tmp_dir: PathBuf) -> Result<Self> {
        let plug_out_dir = tmp_dir.join("out");
        fs::create_dir_all(&plug_out_dir)?;
        let plug_tmp_dir = tmp_dir.join("tmp");
        fs::create_dir_all(&plug_tmp_dir)?;

        let manifest = Manifest::new([Wasm::data(wasm)])
            .with_allowed_path(tmp_dir.to_string_lossy().to_string(), "/");

        let plugin = extism::Plugin::new(WasmInput::Manifest(manifest), renderer_imports(), true)
            .map_err(|e| anyhow!(e).context("加载 extism 渲染器失败"))?;

        if !plugin.function_exists(&function) {
            bail!("插件未导出函数：{}", function);
        }

        Ok(Self {
            plugin: Mutex::new(plugin),
            function,
            out_dir: plug_out_dir,
            tmp_dir,
        })
    }
}

impl Renderer for ExtismRenderer {
    fn render(
        &self,
        doc: &RenderDocument,
        assets: Box<dyn AssetProvider>,
    ) -> Result<(PathBuf, Vec<OutputFile>)> {
        if self.out_dir.exists() {
            fs::remove_dir_all(&self.out_dir)?;
        }
        fs::create_dir_all(&self.out_dir)?;

        let ctx = RenderContext {
            assets,
            streams: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(0),
            tmp_dir: self.tmp_dir.clone(),
        };

        let mut plugin = self
            .plugin
            .lock()
            .map_err(|e| anyhow!("插件锁中毒：{}", e))?;
        let output: Json<RendererOutput> = plugin
            .call_with_host_context(&self.function, Json(doc), ctx)
            .map_err(|e| anyhow!(e).context("调用 extism 渲染器失败"))?;

        let files = collect_outputs(&self.out_dir)?;

        Ok((output.0.main, files))
    }
}

/// 递归扫描产物目录，转成 `OutputFile` 文件树（文件 + 空目录）。
fn collect_outputs(base: &Path) -> Result<Vec<OutputFile>> {
    fn walk(dir: &Path, rel: &Path, out: &mut Vec<OutputFile>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let rel_path = rel.join(entry.file_name());
            if path.is_dir() {
                walk(&path, &rel_path, out)?;
                out.push(OutputFile::Dir(rel_path));
            } else {
                let file = fs::File::open(&path)?;
                out.push(OutputFile::File {
                    path: rel_path,
                    bytes: Box::new(file),
                });
            }
        }
        Ok(())
    }

    let mut out = Vec::new();
    walk(base, Path::new(""), &mut out)?;
    Ok(out)
}
