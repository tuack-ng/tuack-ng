use crate::prelude::*;
use crate::utils::aligned::AlignedFields;
use clap::{Args, Subcommand};
use owo_colors::OwoColorize;
use tuack_utils::plugin::manager::{DISABLED_MARKER, PluginState, PluginStatus, TRUSTED_MARKER};
use tuack_utils::plugin::manifest::{ComponentBody, ExecutableComponent, PluginManifest};

mod market;

use market::MarketArgs;

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
    List,
    /// 输出单个插件的详细信息
    #[command(version)]
    Status {
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

pub fn main(args: PluginArgs) -> Result<()> {
    match args.command {
        PluginCommands::List => list(),
        PluginCommands::Status { name } => status(&name),
        PluginCommands::Enable { name } => set_disabled(&name, false),
        PluginCommands::Disable { name } => set_disabled(&name, true),
        PluginCommands::Trust { name, yes } => set_trusted(&name, true, yes),
        PluginCommands::Untrust { name } => set_trusted(&name, false, true),
        PluginCommands::Uninstall { name, yes } => uninstall(&name, yes),
        PluginCommands::Market(args) => market::main(args),
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

fn list() -> Result<()> {
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
                    format!("使用 `tuack-ng plugin status {}` 以显示错误信息", s.name).dimmed()
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

fn status(name: &str) -> Result<()> {
    print_status(find(name)?)
}

fn print_status(s: &PluginStatus) -> Result<()> {
    let mut fields = AlignedFields::new();
    fields.push("名称", &s.name);
    fields.push("目录", s.dir.display());
    fields.push("状态", state_label(&s.state));
    if let PluginState::Error(reason) = &s.state {
        fields.push("错误", reason);
    }
    let Some(m) = &s.manifest else {
        fields.push("清单", "(无法解析)");
        msg!("{}", fields.render());
        return Ok(());
    };
    fields.push("版本", &m.version);
    if let Some(x) = &m.description {
        fields.push("描述", x);
    }
    if !m.authors.is_empty() {
        fields.push("作者", m.authors.join(", "));
    }
    if let Some(x) = &m.license {
        fields.push("许可证", x);
    }
    if let Some(x) = &m.repo_url {
        fields.push("仓库", x);
    }
    if let Some(x) = &m.url {
        fields.push("主页", x);
    }
    if let Some(x) = &m.minver {
        fields.push("最低版本", x);
    }
    if let Some(x) = &m.asset_dir {
        fields.push("资源目录", x.display());
    }
    fields.push("组件", "");
    msg!("{}", fields.render());
    for c in &m.components {
        msg!("  {} ({})", c.name.blue(), kind_name(&c.body));
        let mut sub = AlignedFields::new();
        match &c.body {
            ComponentBody::RenTemplate(t) => {
                sub.push("    renderer", &t.renderer);
                let processors = if t.processors.is_empty() {
                    "(无)".to_string()
                } else {
                    t.processors.join(", ")
                };
                sub.push("    processors", processors);
            }
            ComponentBody::Renderer(e) | ComponentBody::Dumper(e) | ComponentBody::Processor(e) => {
                match permissions(e) {
                    Some(p) => sub.push("    权限", p.bright_yellow()),
                    None => sub.push("    权限", "无".dimmed()),
                }
                sub.push("    func", &e.func);
            }
        }
        msg!("{}", sub.render());
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
    if e.command.iter().any(|c| c == "*") {
        parts.push("可执行任意程序".to_string());
    } else if !e.command.is_empty() {
        parts.push(format!("命令 ({})", e.command.join(",")));
    }
    (!parts.is_empty()).then(|| parts.join("，"))
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
        print_status(s)?;
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
