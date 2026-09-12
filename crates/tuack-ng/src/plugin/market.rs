use crate::prelude::*;
use crate::utils::aligned::AlignedFields;
use clap::{Args, Subcommand};
use owo_colors::OwoColorize;
use std::time::Duration;
use tuack_utils::plugin::manager::{DISABLED_MARKER, TRUSTED_MARKER, meets_minver, valid_name};

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
        /// 覆盖非市场安装的本地插件
        #[arg(long)]
        force: bool,
    },
}

const INDEX_URL: &str =
    "https://raw.githubusercontent.com/tuack-ng/tuack-ng-plugins/master/index.json";

/// 市场来源标记：安装时由宿主写入，供更新时识别非同源插件。
const FROM_MARKET_MARKER: &str = ".from-market";
/// 市场索引体大小上限。
const MAX_INDEX_BYTES: u64 = 1024 * 1024;
/// 插件归档下载与解压体积上限。
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
/// 插件归档条目数上限。
const MAX_ENTRIES: usize = 10_000;

/// 访问市场的 HTTP agent：强制 HTTPS，设置连接超时与整体超时。
fn agent(global: Duration) -> ureq::Agent {
    ureq::config::Config::builder()
        .timeout_global(Some(global))
        .timeout_connect(Some(Duration::from_secs(10)))
        .https_only(true)
        .build()
        .new_agent()
}

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

pub(super) fn main(args: MarketArgs) -> Result<()> {
    match args.command {
        MarketCommands::List => market_list(),
        MarketCommands::Show { name } => market_show(&name),
        MarketCommands::Install { name, force } => market_install(&name, force),
        MarketCommands::Update { name, all, force } => market_update(name, all, force),
    }
}

fn fetch_market() -> Result<MarketplaceIndex> {
    let mut response = agent(Duration::from_secs(15))
        .get(INDEX_URL)
        .call()
        .context("获取插件市场索引失败")?;
    let text = response
        .body_mut()
        .with_config()
        .limit(MAX_INDEX_BYTES)
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
    let mut fields = AlignedFields::new();
    fields.push("名称", &p.name);
    fields.push("版本", &p.version);
    if !p.description.is_empty() {
        fields.push("描述", &p.description);
    }
    if !p.authors.is_empty() {
        fields.push("作者", p.authors.join(", "));
    }
    if !p.license.is_empty() {
        fields.push("许可证", &p.license);
    }
    if !p.repo_url.is_empty() {
        fields.push("仓库", &p.repo_url);
    }
    if let Some(url) = &p.url {
        fields.push("主页", url);
    }
    if let Some(minver) = &p.minver {
        fields.push("最低版本", minver);
    }
    msg!("{}", fields.render());
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
fn market_update(name: Option<String>, all: bool, force: bool) -> Result<()> {
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
        if let Err(e) = market_update_one(plugin, force) {
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
fn market_update_one(plugin: &MarketplacePlugin, force: bool) -> Result<()> {
    check_minver(plugin)?;
    let status = gctx().plugins.plugin(&plugin.name)?;
    if !force && !status.dir.join(FROM_MARKET_MARKER).is_file() {
        msg_warn!(
            "{} 不是从市场安装的，跳过（加 `--force` 覆盖）",
            plugin.name
        );
        return Ok(());
    }
    if let Some(manifest) = status.manifest.as_ref() {
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
    install_plugin(plugin, &status.dir)?;
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
    let mut response = agent(Duration::from_secs(600))
        .get(&plugin.download_url)
        .call()
        .context("下载插件失败")?;

    let mut archive = tempfile::Builder::new()
        .prefix("tuack-ng-plugin-")
        .suffix(".zip")
        .tempfile()
        .context("创建临时文件失败")?;
    let body = response.body_mut().with_config().limit(MAX_ARCHIVE_BYTES);
    std::io::copy(&mut body.reader(), archive.as_file_mut()).context("下载插件失败")?;

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
    let was_disabled = had_dest && dest.join(DISABLED_MARKER).exists();
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

    // 移除作者可能误打包的标记（信任 / 禁用 / 市场来源），统一由宿主决定
    for marker in [TRUSTED_MARKER, DISABLED_MARKER, FROM_MARKET_MARKER] {
        let path = dest.join(marker);
        if path.exists() {
            if let Err(e) = fs::remove_file(&path) {
                msg_warn!("无法移除插件包内的 {}：{}", marker, e);
            } else {
                msg_warn!("插件包内含 {}，已移除", marker);
            }
        }
    }

    // 保留用户此前的禁用意图
    if was_disabled {
        fs::write(dest.join(DISABLED_MARKER), "").context("恢复禁用标记失败")?;
    }

    // 记录市场来源，供更新时识别非同源插件
    fs::write(
        dest.join(FROM_MARKET_MARKER),
        format!(
            "{}\n{}\n",
            plugin.download_url,
            plugin.sha256.to_lowercase()
        ),
    )
    .context("写入市场来源标记失败")?;
    Ok(())
}

/// 流式计算 sha256（十六进制小写）。
fn sha256_hex(mut reader: impl std::io::Read) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher).context("计算 sha256 失败")?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// 解包 zip 文件到 `dest`（拒绝越界路径，限制条目数与解压体积）。
fn extract_zip(archive_path: &Path, dest: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(fs::File::open(archive_path)?)?;
    if archive.len() > MAX_ENTRIES {
        bail!("归档条目过多：{}", archive.len());
    }
    fs::create_dir_all(dest)?;
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        total = total.saturating_add(entry.size());
        if total > MAX_ARCHIVE_BYTES {
            bail!("归档解压体积超出上限");
        }
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
