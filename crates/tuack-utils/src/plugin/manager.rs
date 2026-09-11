//! 插件管理器：扫描自带模板与插件包，并按名提供渲染器/导出器实例。
//!
//! - 自带模板：`<assets_dir>/templates/<name>.json`（`TemplateManifest` + store 内容寻址）。
//! - 插件包：`<用户目录>/plugins/<pkg>/plugin.toml`（仅用户目录，见 [`PluginManager::discover`]）。
//!
//! 冲突与失败策略：插件包解析失败、`minver` 不满足、组件名与同域内置/其他插件冲突时，
//! **跳过该包并警告**；自带模板解析失败为硬错误。不同域（dumper/renderer/processor/ren_template）允许同名。

use tempfile::TempDir;
use tuack_lib::dump::Dumper;
use tuack_lib::ren::{RenProcessor, Renderer};

use crate::plugin::manifest::{Component, ComponentBody, ComponentKind, PluginManifest};
use crate::prelude::*;
use crate::ren::manifest::{TargetType, TemplateManifest};

/// 渲染配置（模板默认值，可被工程 day/contest 覆盖）。
#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub use_pretest: bool,
    pub noi_style: bool,
    pub file_io: bool,
}

/// 内置渲染器（按内置名引用）。
#[derive(Debug, Clone)]
pub(crate) enum BuiltinRenderer {
    Typst,
    Markdown,
}

/// 内置导出器（按内置名引用）。
#[derive(Debug, Clone, Copy)]
pub(crate) enum BuiltinDumper {
    Lemon,
    Arbiter,
    CcrPlus,
}

/// 插件组件定位（跨包引用暂不允许，但定位信息保留）。
#[derive(Debug, Clone)]
pub(crate) struct PluginRef {
    pub package: String,
    pub component: String,
}

/// 渲染器引用。
#[derive(Debug, Clone)]
pub(crate) enum RendererRef {
    Builtin(BuiltinRenderer),
    Plugin(PluginRef),
}

/// 处理器引用。
#[derive(Debug, Clone)]
pub(crate) enum ProcessorRef {
    Builtin(String),
    Plugin(PluginRef),
}

/// 导出器引用。
#[derive(Debug, Clone)]
pub(crate) enum DumperRef {
    Builtin(BuiltinDumper),
    Plugin(PluginRef),
}

/// 模板来源。
#[derive(Debug, Clone)]
pub(crate) enum TemplateSource {
    /// 自带模板：从内容寻址 store 解包（`filelist`）
    Store { filelist: IndexMap<String, String> },
    /// 插件模板：包内目录直拷
    Dir(PathBuf),
    /// 无模板文件（空目录）
    Empty,
}

/// 已解析的渲染模板。
#[derive(Debug, Clone)]
pub(crate) struct ResolvedTemplate {
    pub source: TemplateSource,
    pub renderer: RendererRef,
    pub processors: Vec<ProcessorRef>,
    pub use_pretest: bool,
    pub noi_style: bool,
    pub file_io: bool,
}

/// 插件包。
#[allow(dead_code)] // description/license/repo_url/url 暂存，供将来市场功能使用
pub(crate) struct PluginPackage {
    pub name: String,
    pub dir: PathBuf,
    pub description: Option<String>,
    pub license: Option<String>,
    pub repo_url: Option<String>,
    pub url: Option<String>,
    /// 随包资源目录（包内相对路径）
    pub asset_dir: Option<PathBuf>,
    /// wasm 入口文件（包内相对路径）
    pub entry: Option<PathBuf>,
    /// (类型，组件名) -> 组件
    pub components: IndexMap<(ComponentKind, String), Component>,
}

/// 内置模板登记项（惰性解析清单）。
struct BuiltinTemplate {
    path: PathBuf,
}

/// 内置渲染器/处理器/导出器的保留名。
const BUILTIN_RENDERERS: &[&str] = &["typst", "markdown"];
const BUILTIN_PROCESSORS: &[&str] = &["loj_table", "html_table", "uoj_title"];
const BUILTIN_DUMPERS: &[&str] = &["lemon", "arbiter", "ccr-plus"];

/// 插件状态。
#[derive(Debug, Clone)]
pub enum PluginState {
    /// 未信任（没有 `.trusted`，不加载）
    Untrusted,
    /// 已禁用（有 `.disabled`，不加载）
    Disabled,
    /// 已加载（正常）
    Loaded,
    /// 加载失败（原因）
    Error(String),
}

impl PluginState {
    /// 是否加载失败。
    pub fn is_error(&self) -> bool {
        matches!(self, PluginState::Error(_))
    }
}

/// 一个已发现插件的状态。
#[derive(Debug, Clone)]
pub struct PluginStatus {
    /// 包名（清单解析失败时用目录名兜底）
    pub name: String,
    /// 插件目录
    pub dir: PathBuf,
    pub state: PluginState,
    /// 已解析的清单（解析失败或未尝试时为 `None`）
    pub manifest: Option<PluginManifest>,
}

impl PluginStatus {
    /// 失败原因（非错误状态时为 `None`）。
    pub fn reason(&self) -> Option<&str> {
        match &self.state {
            PluginState::Error(reason) => Some(reason),
            _ => None,
        }
    }
}

/// 禁用标记文件名（存在于插件目录时禁用）。
pub const DISABLED_MARKER: &str = ".disabled";
/// 信任标记文件名（存在于插件目录时视为已信任）。
pub const TRUSTED_MARKER: &str = ".trusted";

/// 渲染所需：渲染器、渲染配置与处理器链。
pub type RenderSetup = (Box<dyn Renderer>, RenderOptions, Vec<Box<dyn RenProcessor>>);

/// 插件管理器。
pub struct PluginManager {
    templates: IndexMap<String, BuiltinTemplate>,
    packages: IndexMap<String, PluginPackage>,
    /// 缓存：(类型，组件名) -> 归属包名（组件本体只存于所属包的 `components`）
    component_owner: IndexMap<(ComponentKind, String), String>,
    /// 全部已发现插件的状态（含加载成功与失败）
    statuses: Vec<PluginStatus>,
    /// 资源目录（自带模板 store 查找、arbiter 资源）
    assets_dirs: Vec<PathBuf>,
    /// 用户插件目录
    plugin_dir: PathBuf,
}

impl PluginManager {
    /// 扫描 `assets_dirs`（自带模板）与 `plugin_dir`（插件包，仅用户目录），构建管理器。
    ///
    /// `current_version` 用于 `minver` 校验。各插件加载状态见 [`PluginManager::plugin_statuses`]。
    pub fn discover(assets_dirs: &[PathBuf], plugin_dir: &Path, current_version: &str) -> Self {
        // 1. 自带模板（只登记路径与名字，清单惰性解析）
        let mut templates: IndexMap<String, BuiltinTemplate> = IndexMap::new();
        let mut builtin_template_names: Vec<String> = Vec::new();

        for dir in assets_dirs {
            let tdir = dir.join("templates");
            let Ok(entries) = fs::read_dir(&tdir) else {
                continue;
            };
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        debug!("读取模板目录条目失败（{}）：{e}", tdir.display());
                        continue;
                    }
                };
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Some(name) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                    continue;
                };
                if templates.contains_key(&name) {
                    // 高优先级目录已占用，跳过
                    continue;
                }
                builtin_template_names.push(name.clone());
                templates.insert(name, BuiltinTemplate { path });
            }
        }

        // 2. 插件包
        let mut packages: IndexMap<String, PluginPackage> = IndexMap::new();
        let mut component_owner: IndexMap<(ComponentKind, String), String> = IndexMap::new();
        let mut statuses: Vec<PluginStatus> = Vec::new();

        if let Ok(entries) = fs::read_dir(plugin_dir) {
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        debug!("读取插件目录条目失败（{}）：{e}", plugin_dir.display());
                        continue;
                    }
                };
                let pkg_dir = entry.path();
                if !pkg_dir.is_dir() {
                    continue;
                }
                // 跳过隐藏目录（如安装暂存目录 .staging-*）
                if pkg_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'))
                {
                    continue;
                }
                let dir_name = pkg_dir
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();

                // 解析插件清单
                let manifest_path = pkg_dir.join("plugin.toml");
                let parsed: Result<PluginManifest> = fs::read_to_string(&manifest_path)
                    .map_err(anyhow::Error::from)
                    .and_then(|text| toml::from_str(&text).map_err(anyhow::Error::from));
                let name = match &parsed {
                    Ok(m) => m.name.clone(),
                    Err(_) => dir_name.clone(),
                };

                // 门控：禁用优先，其次未信任；两者均不加载
                let disabled = pkg_dir.join(DISABLED_MARKER).is_file();
                let trusted = pkg_dir.join(TRUSTED_MARKER).is_file();
                if disabled {
                    statuses.push(status(name, &pkg_dir, PluginState::Disabled, parsed.ok()));
                    continue;
                }
                if !trusted {
                    statuses.push(status(name, &pkg_dir, PluginState::Untrusted, parsed.ok()));
                    continue;
                }

                // 已信任且未禁用：尝试加载
                let manifest: PluginManifest = match parsed {
                    Ok(m) => m,
                    Err(e) => {
                        statuses.push(status(
                            dir_name,
                            &pkg_dir,
                            PluginState::Error(format!("解析 plugin.toml 失败：{e}")),
                            None,
                        ));
                        continue;
                    }
                };
                match load_package(
                    &pkg_dir,
                    &dir_name,
                    &manifest,
                    current_version,
                    &builtin_template_names,
                    &component_owner,
                    &packages,
                ) {
                    Ok(package) => {
                        for key in package.components.keys() {
                            component_owner.insert(key.clone(), package.name.clone());
                        }
                        statuses.push(status(
                            package.name.clone(),
                            &pkg_dir,
                            PluginState::Loaded,
                            Some(manifest),
                        ));
                        packages.insert(package.name.clone(), package);
                    }
                    Err((status_name, reason)) => {
                        statuses.push(status(
                            status_name,
                            &pkg_dir,
                            PluginState::Error(reason),
                            Some(manifest),
                        ));
                    }
                }
            }
        }

        Self {
            templates,
            packages,
            component_owner,
            statuses,
            assets_dirs: assets_dirs.to_vec(),
            plugin_dir: plugin_dir.to_path_buf(),
        }
    }

    /// 全部已发现插件的状态（含加载成功与失败）。
    pub fn plugin_statuses(&self) -> &[PluginStatus] {
        &self.statuses
    }

    /// 按包名查找插件状态（清单解析失败时 `name` 为目录名兜底）。
    pub fn plugin(&self, name: &str) -> Result<&PluginStatus> {
        self.statuses
            .iter()
            .find(|s| s.name == name)
            .context(format!("未找到插件：{}", name))
    }

    /// 加载失败（`Error`）的插件。
    pub fn failed_plugins(&self) -> impl Iterator<Item = &PluginStatus> {
        self.statuses.iter().filter(|s| s.state.is_error())
    }

    /// 用户插件目录。
    pub fn plugins_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// 模板名是否存在（自带模板或插件 `ren_template` 组件）。
    pub fn template_exists(&self, name: &str) -> bool {
        self.templates.contains_key(name)
            || self
                .component_owner
                .contains_key(&(ComponentKind::RenTemplate, name.to_string()))
    }

    /// 可用模板名（自带模板 + 插件 `ren_template` 组件），已排序去重。
    pub fn template_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.templates.keys().cloned().collect();
        names.extend(self.component_names(ComponentKind::RenTemplate));
        names.sort();
        names.dedup();
        names
    }

    /// 可用导出器名（内置 + 插件 `dumper` 组件），已排序去重。
    pub fn dumper_names(&self) -> Vec<String> {
        let mut names: Vec<String> = BUILTIN_DUMPERS.iter().map(|s| s.to_string()).collect();
        names.extend(self.component_names(ComponentKind::Dumper));
        names.sort();
        names.dedup();
        names
    }

    /// 某域下所有已加载插件组件的名字。
    fn component_names(&self, kind: ComponentKind) -> impl Iterator<Item = String> + '_ {
        self.component_owner
            .keys()
            .filter(move |(k, _)| *k == kind)
            .map(|(_, name)| name.clone())
    }

    pub(crate) fn package(&self, name: &str) -> Option<&PluginPackage> {
        self.packages.get(name)
    }

    /// 构造渲染器、渲染配置与处理器链；会先把模板文件落到 `tmp/tmp`。
    pub fn renderer(&self, name: &str, tmp: Arc<TempDir>) -> Result<RenderSetup> {
        let template = self.resolve_template(name)?;
        let options = RenderOptions {
            use_pretest: template.use_pretest,
            noi_style: template.noi_style,
            file_io: template.file_io,
        };
        let renderer =
            crate::plugin::factory::build_renderer(&template, self, tmp, &self.assets_dirs)?;
        let processors = template
            .processors
            .iter()
            .map(|p| crate::plugin::factory::build_processor(p, self))
            .collect::<Result<Vec<_>>>()?;
        Ok((renderer, options, processors))
    }

    /// 构造导出器。
    pub fn dumper(&self, name: &str, tmp: Arc<TempDir>) -> Result<Box<dyn Dumper>> {
        let dumper_ref = self.resolve_dumper(name)?;
        crate::plugin::factory::build_dumper(&dumper_ref, self, tmp, &self.assets_dirs)
    }

    /// 按 (类型，组件名) 查找所属包与组件。
    fn find_component(
        &self,
        kind: ComponentKind,
        name: &str,
    ) -> Option<(&PluginPackage, &Component)> {
        let key = (kind, name.to_string());
        let pkg_name = self.component_owner.get(&key)?;
        let package = self.packages.get(pkg_name)?;
        Some((package, package.components.get(&key)?))
    }

    /// 解析渲染模板：自带模板名，或插件 `ren_template` 组件名。
    fn resolve_template(&self, name: &str) -> Result<ResolvedTemplate> {
        if self.templates.contains_key(name) {
            return self.template_from_builtin(name);
        }
        if self
            .component_owner
            .contains_key(&(ComponentKind::RenTemplate, name.to_string()))
        {
            return self.template_from_plugin(name);
        }
        bail!("没有找到模板 {}", name)
    }

    /// 从自带清单（store）构造模板 spec。
    fn template_from_builtin(&self, name: &str) -> Result<ResolvedTemplate> {
        let builtin = self.templates.get(name).expect("调用方已确认存在");
        let manifest: TemplateManifest = serde_json::from_str(&fs::read_to_string(&builtin.path)?)?;
        let renderer = match manifest.target {
            TargetType::Typst => RendererRef::Builtin(BuiltinRenderer::Typst),
            TargetType::Markdown => RendererRef::Builtin(BuiltinRenderer::Markdown),
        };
        let processors = manifest
            .processor
            .iter()
            .map(|p| ProcessorRef::Builtin(p.clone()))
            .collect();
        Ok(ResolvedTemplate {
            source: TemplateSource::Store {
                filelist: manifest.filelist.clone(),
            },
            renderer,
            processors,
            use_pretest: manifest.use_pretest,
            noi_style: manifest.noi_style,
            file_io: manifest.file_io,
        })
    }

    /// 从插件 `ren_template` 组件构造模板 spec。
    fn template_from_plugin(&self, comp_name: &str) -> Result<ResolvedTemplate> {
        let (package, comp) = self
            .find_component(ComponentKind::RenTemplate, comp_name)
            .context(format!("未找到插件组件：{}", comp_name))?;
        let ComponentBody::RenTemplate(t) = &comp.body else {
            bail!("组件 {} 不是 ren_template", comp_name);
        };
        let source = match &t.template {
            Some(dir) => TemplateSource::Dir(package.dir.join(dir)),
            None => TemplateSource::Empty,
        };
        let renderer = self.parse_renderer(&t.renderer, &package.name)?;
        let processors = t
            .processors
            .iter()
            .map(|p| self.parse_processor(p, &package.name))
            .collect::<Result<Vec<_>>>()?;
        Ok(ResolvedTemplate {
            source,
            renderer,
            processors,
            use_pretest: t.use_pretest,
            noi_style: t.noi_style,
            file_io: t.file_io,
        })
    }

    /// 解析导出器：内置名（`lemon` / `arbiter` / `ccr-plus`），或插件 `dumper` 组件名。
    fn resolve_dumper(&self, name: &str) -> Result<DumperRef> {
        let builtin = match name {
            "lemon" => Some(BuiltinDumper::Lemon),
            "arbiter" => Some(BuiltinDumper::Arbiter),
            "ccr-plus" => Some(BuiltinDumper::CcrPlus),
            _ => None,
        };
        if let Some(builtin) = builtin {
            return Ok(DumperRef::Builtin(builtin));
        }
        if self
            .component_owner
            .contains_key(&(ComponentKind::Dumper, name.to_string()))
        {
            return self.dumper_from_plugin(name);
        }
        bail!("没有找到导出目标 {}", name)
    }

    /// 从插件 `dumper` 组件构造导出器引用。
    fn dumper_from_plugin(&self, comp_name: &str) -> Result<DumperRef> {
        let (package, comp) = self
            .find_component(ComponentKind::Dumper, comp_name)
            .context(format!("未找到插件组件：{}", comp_name))?;
        if !matches!(comp.body, ComponentBody::Dumper(_)) {
            bail!("组件 {} 不是 dumper", comp_name);
        }
        Ok(DumperRef::Plugin(PluginRef {
            package: package.name.clone(),
            component: comp_name.to_string(),
        }))
    }

    /// 解析渲染器：内置名（`typst` / `markdown`），或本包 `renderer` 组件名。
    fn parse_renderer(&self, s: &str, pkg: &str) -> Result<RendererRef> {
        match s {
            "typst" => return Ok(RendererRef::Builtin(BuiltinRenderer::Typst)),
            "markdown" => return Ok(RendererRef::Builtin(BuiltinRenderer::Markdown)),
            _ => {}
        }
        let (package, _) = self
            .find_component(ComponentKind::Renderer, s)
            .with_context(|| format!("未知渲染器：{}", s))?;
        if package.name != pkg {
            bail!("不允许跨包引用：{} 引用了包 {} 的组件", pkg, package.name);
        }
        Ok(RendererRef::Plugin(PluginRef {
            package: package.name.clone(),
            component: s.to_string(),
        }))
    }

    /// 解析处理器：内置名，或本包 `processor` 组件名。
    fn parse_processor(&self, s: &str, pkg: &str) -> Result<ProcessorRef> {
        if BUILTIN_PROCESSORS.contains(&s) {
            return Ok(ProcessorRef::Builtin(s.to_string()));
        }
        let (package, _) = self
            .find_component(ComponentKind::Processor, s)
            .with_context(|| format!("未知处理器：{}", s))?;
        if package.name != pkg {
            bail!("不允许跨包引用：{} 引用了包 {} 的组件", pkg, package.name);
        }
        Ok(ProcessorRef::Plugin(PluginRef {
            package: package.name.clone(),
            component: s.to_string(),
        }))
    }
}

/// 构造插件状态项。
fn status(
    name: impl Into<String>,
    dir: &Path,
    state: PluginState,
    manifest: Option<PluginManifest>,
) -> PluginStatus {
    PluginStatus {
        name: name.into(),
        dir: dir.to_path_buf(),
        state,
        manifest,
    }
}

/// 合法的包名 / 组件名（仅 ASCII 字母数字与 `-`/`_`，避免路径分隔与 `..`）。
pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// 组件与文件的可访问性校验：有可执行组件必须有 entry；声明的模板目录必须存在。
fn component_io_error(manifest: &PluginManifest, pkg_dir: &Path) -> Option<String> {
    let has_executable = manifest.components.iter().any(|c| {
        matches!(
            c.body,
            ComponentBody::Dumper(_) | ComponentBody::Renderer(_) | ComponentBody::Processor(_)
        )
    });
    if has_executable && manifest.entry.is_none() {
        return Some("有可执行组件但未声明 entry".to_string());
    }
    for c in &manifest.components {
        if let ComponentBody::RenTemplate(t) = &c.body
            && let Some(dir) = &t.template
            && !pkg_dir.join(dir).is_dir()
        {
            return Some(format!("模板目录不存在：{}", dir.display()));
        }
    }
    None
}

/// 校验受信任插件的清单并收集组件，产出可入库的包。
///
/// 失败返回 `(状态名，原因)`，由调用方登记为 `Error` 状态。
fn load_package(
    pkg_dir: &Path,
    dir_name: &str,
    manifest: &PluginManifest,
    current_version: &str,
    builtin_template_names: &[String],
    component_owner: &IndexMap<(ComponentKind, String), String>,
    existing_packages: &IndexMap<String, PluginPackage>,
) -> std::result::Result<PluginPackage, (String, String)> {
    let name = manifest.name.clone();

    if !valid_name(&name) {
        return Err((dir_name.to_string(), format!("非法包名：{}", name)));
    }
    if existing_packages.contains_key(&name) {
        return Err((name, "包名重复".to_string()));
    }
    if let Err(e) = semver::Version::parse(&manifest.version) {
        return Err((name, format!("插件 version 非法：{e}")));
    }
    if let Some(minver) = manifest.minver.clone() {
        match meets_minver(current_version, &minver) {
            Ok(true) => {}
            Ok(false) => {
                return Err((
                    name,
                    format!("要求主程序 >= {}，当前 {}", minver, current_version),
                ));
            }
            Err(e) => return Err((name, e.to_string())),
        }
    }

    // 验证 wasm 入口与资源目录可访问
    if let Some(entry) = manifest.entry.as_ref()
        && !pkg_dir.join(entry).is_file()
    {
        return Err((name, format!("entry 不存在：{}", entry.display())));
    }
    if let Some(asset_dir) = manifest.asset_dir.as_ref()
        && !pkg_dir.join(asset_dir).is_dir()
    {
        return Err((name, format!("asset_dir 不可访问：{}", asset_dir.display())));
    }
    if let Some(reason) = component_io_error(manifest, pkg_dir) {
        return Err((name, reason));
    }

    let components = collect_components(manifest, builtin_template_names, component_owner)
        .map_err(|reason| (name.clone(), reason))?;

    Ok(PluginPackage {
        name,
        dir: pkg_dir.to_path_buf(),
        description: manifest.description.clone(),
        license: manifest.license.clone(),
        repo_url: manifest.repo_url.clone(),
        url: manifest.url.clone(),
        asset_dir: manifest.asset_dir.clone(),
        entry: manifest.entry.clone(),
        components,
    })
}

/// 收集并校验一个插件包的组件（同域内包内唯一 + 不与内置/已登记组件冲突）。
fn collect_components(
    manifest: &PluginManifest,
    builtin_template_names: &[String],
    existing: &IndexMap<(ComponentKind, String), String>,
) -> std::result::Result<IndexMap<(ComponentKind, String), Component>, String> {
    let mut components: IndexMap<(ComponentKind, String), Component> = IndexMap::new();
    for comp in &manifest.components {
        if !valid_name(&comp.name) {
            return Err(format!("非法组件名：{}", comp.name));
        }
        let key = (comp.body.kind(), comp.name.clone());
        if components.contains_key(&key) {
            return Err(format!("同域组件重名：{}", comp.name));
        }
        if is_builtin_reserved(key.0, &comp.name, builtin_template_names) {
            return Err(format!("组件名与内置冲突：{}", comp.name));
        }
        if existing.contains_key(&key) {
            return Err(format!("组件名与其他插件冲突：{}", comp.name));
        }
        components.insert(key, comp.clone());
    }
    Ok(components)
}

/// 该 (类型，名字) 是否占用内置保留名。
fn is_builtin_reserved(kind: ComponentKind, name: &str, builtin_template_names: &[String]) -> bool {
    match kind {
        ComponentKind::RenTemplate => builtin_template_names.iter().any(|n| n == name),
        ComponentKind::Renderer => BUILTIN_RENDERERS.contains(&name),
        ComponentKind::Processor => BUILTIN_PROCESSORS.contains(&name),
        ComponentKind::Dumper => BUILTIN_DUMPERS.contains(&name),
    }
}

/// 语义化版本比较：`current >= required`。
pub fn meets_minver(current: &str, required: &str) -> Result<bool> {
    let current = semver::Version::parse(current)
        .with_context(|| format!("主程序版本号非法：{}", current))?;
    let required = semver::Version::parse(required)
        .with_context(|| format!("插件 minver 非法：{}", required))?;
    Ok(current >= required)
}
