use crate::prelude::*;
use clap::{Args, Subcommand};
use owo_colors::OwoColorize;
use tuack_utils::plugin::manager::{
    DISABLED_MARKER, PluginState, PluginStatus, TRUSTED_MARKER, meets_minver, valid_name,
};
use tuack_utils::plugin::manifest::{ComponentBody, ExecutableComponent, PluginManifest};
use unicode_width::UnicodeWidthStr;

#[derive(Args, Debug)]
#[command(version)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommands,
}

#[derive(Subcommand, Debug)]
#[command(version)]
pub enum PluginCommands {
    /// 列出所有插件与状态
    #[command(version)]
    Status,
    /// 输出单个插件的详细信息
    #[command(version)]
    Show {
        /// 插件名（包名或目录名）
        name: String,
    },
    /// 启用插件
    #[command(version)]
    Enable { name: String },
    /// 禁用插件
    #[command(version)]
    Disable { name: String },
    /// 信任插件
    #[command(version)]
    Trust {
        name: String,
        /// 跳过确认（不推荐）
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// 取消信任
    #[command(version)]
    Untrust { name: String },
    /// 卸载插件
    #[command(version)]
    Uninstall {
        name: String,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// 插件市场（浏览 / 安装 / 更新）
    #[command(version)]
    Market(MarketArgs),
}

#[derive(Args, Debug)]
#[command(version)]
pub struct MarketArgs {
    #[command(subcommand)]
    pub command: MarketCommands,
}

#[derive(Subcommand, Debug)]
#[command(version)]
pub enum MarketCommands {
    /// 列出市场中的插件
    #[command(version)]
    List,
    /// 输出市场中单个插件的详细信息
    #[command(version)]
    Show {
        /// 插件名
        name: String,
    },
    /// 从市场安装
    #[command(version)]
    Install {
        /// 插件名
        name: String,
        /// 覆盖已存在的目录
        #[arg(long)]
        force: bool,
    },
    /// 更新已安装插件（更新后取消信任）
    #[command(version)]
    Update {
        /// 插件名（省略时需加 `--all`）
        name: Option<String>,
        /// 更新全部已安装插件
        #[arg(long)]
        all: bool,
    },
}

pub fn main(args: PluginArgs) -> Result<()> {
    match args.command {
        PluginCommands::Status => status(),
        PluginCommands::Show { name } => show(&name),
        PluginCommands::Enable { name } => set_disabled(&name, false),
        PluginCommands::Disable { name } => set_disabled(&name, true),
        PluginCommands::Trust { name, yes } => set_trusted(&name, true, yes),
        PluginCommands::Untrust { name } => set_trusted(&name, false, true),
        PluginCommands::Uninstall { name, yes } => uninstall(&name, yes),
        PluginCommands::Market(args) => market_main(args),
    }
}

fn market_main(args: MarketArgs) -> Result<()> {
    match args.command {
        MarketCommands::List => market_list(),
        MarketCommands::Show { name } => market_show(&name),
        MarketCommands::Install { name, force } => market_install(&name, force),
        MarketCommands::Update { name, all } => market_update(name, all),
    }
}

/// 状态标签（带颜色）。
fn state_label(state: &PluginState) -> String {
    match state {
        PluginState::Loaded => "已加载".green().to_string(),
        PluginState::Untrusted => "未信任".yellow().to_string(),
        PluginState::Disabled => "已禁用".dimmed().to_string(),
        PluginState::Error(_) => "错误".red().to_string(),
    }
}

/// 按包名查找插件。
fn find(name: &str) -> Result<&'static PluginStatus> {
    gctx().plugins.plugin(name)
}

fn status() -> Result<()> {
    let statuses = gctx().plugins.plugin_statuses();
    if statuses.is_empty() {
        msg_info!("没有已发现的插件");
        return Ok(());
    }
    for s in statuses {
        msg!("{} {}", versioned(s).blue(), state_label(&s.state));
        msg!("    {}", describe(s.manifest.as_ref()));
        match &s.state {
            PluginState::Error(_) => {
                msg!(
                    "    {}",
                    format!("使用 `tuack-ng plugin show {}` 以显示错误信息", s.name).dimmed()
                );
            }
            PluginState::Untrusted => {
                msg!(
                    "    {}",
                    format!("使用 `tuack-ng plugin trust {}` 以信任并加载", s.name).dimmed()
                );
            }
            _ => {}
        }
    }
    Ok(())
}

/// 描述行：清单缺失 -> 错误提示；无描述 -> 斜体占位。
fn describe(manifest: Option<&PluginManifest>) -> String {
    match manifest {
        None => "插件描述文件错误或不存在".italic().to_string(),
        Some(m) => match m.description.as_deref().filter(|d| !d.is_empty()) {
            Some(d) => d.to_string(),
            None => "（未提供描述）".italic().to_string(),
        },
    }
}

/// 状态行首列：`ID (版本号)`；无清单时仅 `ID`。
fn versioned(s: &PluginStatus) -> String {
    match s.manifest.as_ref() {
        Some(m) => format!("{} ({})", s.name, m.version),
        None => s.name.clone(),
    }
}

fn show(name: &str) -> Result<()> {
    show_status(find(name)?)
}

fn show_status(s: &PluginStatus) -> Result<()> {
    field("名称     ", &s.name);
    field("目录     ", s.dir.display());
    field("状态     ", state_label(&s.state));
    if let PluginState::Error(reason) = &s.state {
        field("错误     ", reason);
    }
    let Some(m) = &s.manifest else {
        field("清单     ", "(无法解析)");
        return Ok(());
    };
    field("版本     ", &m.version);
    if let Some(x) = &m.description {
        field("描述     ", x);
    }
    if !m.authors.is_empty() {
        field("作者     ", m.authors.join(", "));
    }
    if let Some(x) = &m.license {
        field("许可证   ", x);
    }
    if let Some(x) = &m.repo_url {
        field("仓库     ", x);
    }
    if let Some(x) = &m.url {
        field("主页     ", x);
    }
    if let Some(x) = &m.minver {
        field("最低版本 ", x);
    }
    if let Some(x) = &m.asset_dir {
        field("资源目录 ", x.display());
    }
    msg!("{}", "组件     :".dimmed());
    for c in &m.components {
        msg!("  {} ({})", c.name.blue(), kind_name(&c.body));
        match &c.body {
            ComponentBody::RenTemplate(t) => {
                field("    renderer   ", &t.renderer);
                let processors = if t.processors.is_empty() {
                    "(无)".to_string()
                } else {
                    t.processors.join(", ")
                };
                field("    processors ", processors);
            }
            ComponentBody::Renderer(e) | ComponentBody::Dumper(e) | ComponentBody::Processor(e) => {
                match permissions(e) {
                    Some(p) => field("    权限       ", p.bright_yellow()),
                    None => field("    权限       ", "无".dimmed()),
                }
                field("    func       ", &e.func);
            }
        }
    }
    Ok(())
}

/// 组件类型名。
fn kind_name(body: &ComponentBody) -> &'static str {
    match body {
        ComponentBody::Renderer(_) => "renderer",
        ComponentBody::Dumper(_) => "dumper",
        ComponentBody::Processor(_) => "processor",
        ComponentBody::RenTemplate(_) => "ren_template",
    }
}

/// 可执行组件的权限一览；无任何权限时为 `None`。
fn permissions(e: &ExecutableComponent) -> Option<String> {
    let mut parts = Vec::new();
    if e.wasi {
        parts.push("WASI".to_string());
    }
    if !e.command.is_empty() {
        parts.push(format!("命令 ({})", e.command.join(",")));
    }
    (!parts.is_empty()).then(|| parts.join("，"))
}

/// 打印 `标签：值`；标签灰色，多行值续行按前缀宽度缩进对齐。
fn field(label: &str, value: impl std::fmt::Display) {
    let value = value.to_string();
    let prefix = format!("{}:", label);
    let indent = " ".repeat(prefix.width() + 1);
    let mut lines = value.lines();
    let first = lines.next().unwrap_or("");
    msg!("{} {}", prefix.dimmed(), first);
    for line in lines {
        msg!("{}{}", indent, line);
    }
}

fn set_disabled(name: &str, disabled: bool) -> Result<()> {
    let s = find(name)?;
    let path = s.dir.join(DISABLED_MARKER);
    if disabled {
        fs::write(&path, "")?;
        msg_info!("已禁用插件：{}", s.name);
    } else if path.exists() {
        fs::remove_file(&path)?;
        msg_info!("已启用插件：{}", s.name);
    } else {
        msg_info!("插件已是启用状态：{}", s.name);
    }
    Ok(())
}

fn set_trusted(name: &str, trusted: bool, yes: bool) -> Result<()> {
    let s = find(name)?;
    let path = s.dir.join(TRUSTED_MARKER);
    if trusted {
        if path.exists() {
            msg_info!("插件已是信任状态：{}", s.name);
            return Ok(());
        }
        show_status(s)?;
        if !confirm(
            &format!(
                "信任插件 `{}`？插件可执行任意代码，请确认其来源可信",
                s.name
            ),
            yes,
        )? {
            msg_info!("已取消");
            return Ok(());
        }
        fs::write(&path, "")?;
        msg_info!("已信任插件：{}", s.name);
    } else {
        if path.exists() {
            fs::remove_file(&path)?;
            msg_info!("已取消信任：{}", s.name);
        } else {
            msg_info!("插件未处于信任状态：{}", s.name);
        }
    }
    Ok(())
}

fn uninstall(name: &str, yes: bool) -> Result<()> {
    let s = find(name)?;
    let dir = s.dir.clone();
    let pkg = s.name.clone();
    if !confirm(
        &format!("卸载插件 `{}`（删除 {}）？", pkg, dir.display()),
        yes,
    )? {
        msg_info!("已取消");
        return Ok(());
    }
    fs::remove_dir_all(&dir).with_context(|| format!("删除插件目录失败：{}", dir.display()))?;
    msg_info!("已卸载插件：{}", pkg);
    Ok(())
}

/// 确认（`yes` 为真时跳过）。
fn confirm(prompt: &str, yes: bool) -> Result<bool> {
    use std::io::IsTerminal;
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        bail!("非交互终端：请加 `-y` 显式确认（不推荐）");
    }
    Ok(
        dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(prompt)
            .default(false)
            .interact()?,
    )
}

// ---- 市场 ----

const INDEX_URL: &str =
    "https://raw.githubusercontent.com/tuack-ng/tuack-ng-plugins/master/index.json";

#[derive(Debug, Deserialize)]
struct MarketplaceIndex {
    plugins: Vec<MarketplacePlugin>,
}

/// 市场索引条目（字段与 plugin.toml 对齐，另加市场信息）。
#[derive(Debug, Clone, Deserialize)]
struct MarketplacePlugin {
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(default)]
    license: String,
    #[serde(default)]
    repo_url: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    minver: Option<String>,
    artifact_name: String,
    download_url: String,
    sha256: String,
}

fn fetch_market() -> Result<MarketplaceIndex> {
    let mut response = ureq::get(INDEX_URL)
        .call()
        .context("获取插件市场索引失败")?;
    let text = response
        .body_mut()
        .read_to_string()
        .context("读取插件市场索引失败")?;
    serde_json::from_str(&text).context("解析插件市场索引失败")
}

fn market_list() -> Result<()> {
    let index = fetch_market()?;
    if index.plugins.is_empty() {
        msg_info!("市场暂无插件");
        return Ok(());
    }
    for p in &index.plugins {
        msg!("{}", format!("{} ({})", p.name, p.version).blue());
        if !p.description.is_empty() {
            msg!("    {}", p.description);
        }
    }
    Ok(())
}

/// 输出市场中单个插件的详细信息。
fn market_show(name: &str) -> Result<()> {
    let index = fetch_market()?;
    let p = index
        .plugins
        .iter()
        .find(|p| p.name == name)
        .with_context(|| format!("市场中未找到：{}", name))?;
    field("名称     ", &p.name);
    field("版本     ", &p.version);
    if !p.description.is_empty() {
        field("描述     ", &p.description);
    }
    if !p.authors.is_empty() {
        field("作者     ", p.authors.join(", "));
    }
    if !p.license.is_empty() {
        field("许可证   ", &p.license);
    }
    if !p.repo_url.is_empty() {
        field("仓库     ", &p.repo_url);
    }
    if let Some(url) = &p.url {
        field("主页     ", url);
    }
    if let Some(minver) = &p.minver {
        field("最低版本 ", minver);
    }
    Ok(())
}

fn market_install(name: &str, force: bool) -> Result<()> {
    let index = fetch_market()?;
    let plugin = index
        .plugins
        .iter()
        .find(|p| p.name == name)
        .with_context(|| format!("市场中未找到：{}", name))?;

    check_minver(plugin)?;

    let dest = gctx().plugins.plugins_dir().join(name);
    if dest.exists() && !force {
        bail!("插件目录已存在：{}（加 `--force` 覆盖）", dest.display());
    }

    install_plugin(plugin, &dest)?;
    msg_info!(
        "已安装插件 {} 到 {}",
        format!("{} ({})", plugin.name, plugin.version).blue(),
        dest.display().dimmed()
    );
    msg_warn!(
        "新安装的插件默认未信任且不会加载，运行 `tuack-ng plugin trust {}` 以信任并加载",
        plugin.name
    );
    Ok(())
}

/// 更新已安装插件：指定单个插件名，或 `--all` 更新全部。更新后取消信任。
fn market_update(name: Option<String>, all: bool) -> Result<()> {
    if all == name.is_some() {
        bail!("请指定插件名，或用 `--all` 更新全部");
    }
    let index = fetch_market()?;

    let installed: Vec<String> = gctx()
        .plugins
        .plugin_statuses()
        .iter()
        .map(|s| s.name.clone())
        .collect();

    let mut targets: Vec<&MarketplacePlugin> = Vec::new();
    if all {
        // 遍历已安装插件，不在市场中的跳过
        for plugin_name in &installed {
            match index.plugins.iter().find(|p| &p.name == plugin_name) {
                Some(plugin) => targets.push(plugin),
                None => msg_info!("{} 不在市场中，跳过", plugin_name),
            }
        }
    } else {
        let plugin_name = name.as_deref().expect("已校验 name 存在");
        if !installed.iter().any(|n| n == plugin_name) {
            bail!("插件未安装，无法更新：{}", plugin_name);
        }
        match index.plugins.iter().find(|p| p.name == plugin_name) {
            Some(plugin) => targets.push(plugin),
            None => {
                msg_info!("{} 不在市场中，跳过", plugin_name);
                return Ok(());
            }
        }
    }

    if targets.is_empty() {
        msg_info!("没有可更新的插件");
        return Ok(());
    }

    let mut failed: Vec<String> = Vec::new();
    for plugin in targets {
        if let Err(e) = market_update_one(plugin) {
            msg_error!("插件 {} 更新失败：{:?}", plugin.name, e);
            failed.push(plugin.name.clone());
        }
    }
    if !failed.is_empty() {
        bail!("以下插件更新失败：{}", failed.join(", "));
    }
    Ok(())
}

/// 更新单个插件（更新后取消信任）。
fn market_update_one(plugin: &MarketplacePlugin) -> Result<()> {
    check_minver(plugin)?;
    let status = gctx().plugins.plugin(&plugin.name).ok();
    if let Some(manifest) = status.and_then(|s| s.manifest.as_ref()) {
        match version_cmp(&plugin.version, &manifest.version)? {
            std::cmp::Ordering::Equal => {
                msg_info!("{} 已是最新版本 ({})，跳过", plugin.name, plugin.version);
                return Ok(());
            }
            std::cmp::Ordering::Less => {
                msg_warn!(
                    "{} 本地版本 {} 高于市场 {}，跳过",
                    plugin.name,
                    manifest.version,
                    plugin.version
                );
                return Ok(());
            }
            std::cmp::Ordering::Greater => {}
        }
    }
    // 已安装则用其实际目录（目录名可能不同于包名）
    let dest = status
        .map(|s| s.dir.clone())
        .unwrap_or_else(|| gctx().plugins.plugins_dir().join(&plugin.name));
    install_plugin(plugin, &dest)?;
    msg_info!(
        "已更新插件 {} 并取消信任；运行 `tuack-ng plugin trust {}` 重新信任",
        format!("{} ({})", plugin.name, plugin.version).blue(),
        plugin.name
    );
    Ok(())
}

/// 比较市场版本与已安装版本（均为 semver）。
fn version_cmp(market: &str, installed: &str) -> Result<std::cmp::Ordering> {
    let market =
        semver::Version::parse(market).with_context(|| format!("市场版本号非法：{}", market))?;
    let installed = semver::Version::parse(installed)
        .with_context(|| format!("已安装版本号非法：{}", installed))?;
    Ok(market.cmp(&installed))
}

/// 校验主程序版本满足插件 `minver`。
fn check_minver(plugin: &MarketplacePlugin) -> Result<()> {
    let Some(minver) = &plugin.minver else {
        return Ok(());
    };
    if !meets_minver(env!("CARGO_PKG_VERSION"), minver)? {
        bail!(
            "插件 {} 要求主程序 >= {}，当前 {}",
            plugin.name,
            minver,
            env!("CARGO_PKG_VERSION")
        );
    }
    Ok(())
}

/// 下载并安装市场插件到 `dest`：先下载校验，再解包到暂存目录，成功后才替换目标。
fn install_plugin(plugin: &MarketplacePlugin, dest: &Path) -> Result<()> {
    if !valid_name(&plugin.name) {
        bail!("非法的插件名：{}", plugin.name);
    }
    msg_info!("下载 {} ...", plugin.download_url.dimmed());
    let mut response = ureq::get(&plugin.download_url)
        .call()
        .context("下载插件失败")?;

    let mut archive = tempfile::Builder::new()
        .prefix("tuack-ng-plugin-")
        .suffix(".zip")
        .tempfile()
        .context("创建临时文件失败")?;
    std::io::copy(&mut response.body_mut().as_reader(), archive.as_file_mut())
        .context("下载插件失败")?;

    let actual = sha256_hex(archive.reopen()?)?;
    if actual != plugin.sha256.to_lowercase() {
        bail!("sha256 校验失败：期望 {}，实际 {}", plugin.sha256, actual);
    }

    // 解包到同目录暂存目录；失败不影响已安装版本，成功后再替换。
    // 替换先备份原目录，rename 失败则还原，避免中途失败丢失插件。
    let parent = dest.parent().context("非法安装路径")?;
    fs::create_dir_all(parent)?;
    let pid = std::process::id();
    let staging = parent.join(format!(".staging-{}-{}", plugin.name, pid));
    let backup = parent.join(format!(".backup-{}-{}", plugin.name, pid));
    let had_dest = dest.exists();
    let mut backup_kept = false;
    let staged = (|| -> Result<()> {
        extract_zip(archive.path(), &staging)
            .with_context(|| format!("解包插件失败：{}", plugin.artifact_name))?;
        if had_dest {
            fs::rename(dest, &backup).context("备份原插件目录失败")?;
        }
        if let Err(e) = fs::rename(&staging, dest) {
            // 还原失败时保留备份，避免丢失唯一的原插件副本
            if had_dest && fs::rename(&backup, dest).is_err() {
                backup_kept = true;
            }
            return Err(e).context("替换插件目录失败");
        }
        Ok(())
    })();
    if staged.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    if backup_kept {
        msg_error!(
            "原插件目录已保留在 {}，请手动恢复",
            backup.display().to_string().dimmed()
        );
    } else {
        let _ = fs::remove_dir_all(&backup);
    }
    staged?;

    // 移除作者可能误打包的本地标记，避免安装后被自动信任/禁用
    for marker in [TRUSTED_MARKER, DISABLED_MARKER] {
        let path = dest.join(marker);
        if path.exists() {
            if let Err(e) = fs::remove_file(&path) {
                msg_warn!("无法移除插件包内的 {}：{}", marker, e);
            } else {
                msg_warn!("插件包内含 {}，已移除", marker);
            }
        }
    }
    Ok(())
}

/// 流式计算 sha256（十六进制小写）。
fn sha256_hex(mut reader: impl std::io::Read) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher).context("计算 sha256 失败")?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// 解包 zip 文件到 `dest`（拒绝越界路径）。
fn extract_zip(archive_path: &Path, dest: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(fs::File::open(archive_path)?)?;
    fs::create_dir_all(dest)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            bail!("归档内非法路径：{}", entry.name());
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut file)?;
        }
    }
    Ok(())
}
