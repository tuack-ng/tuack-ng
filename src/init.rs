use crate::prelude::*;
use log::LevelFilter;
use log4rs::append::console::Target;
use log4rs::{
    Logger,
    append::console::ConsoleAppender,
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};

use crate::config::msgs::LoadContext;
use crate::{config::load_config, context};
use chrono::Local;
use indicatif::{MultiProgress, ProgressDrawTarget};
use indicatif_log_bridge::LogWrapper;
use owo_colors::OwoColorize;
use std::panic::{self, PanicHookInfo};

#[cfg(debug_assertions)]
const DEBUG: bool = true;
#[cfg(not(debug_assertions))]
const DEBUG: bool = false;

fn custom_panic_handler(panic_info: &PanicHookInfo, verbose: bool) {
    macro_rules! panic_log {
        ($($arg:tt)*) => {
            if verbose {
                eprintln!("{} | {} | {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                "PANIC".bright_red().bold(), format!($($arg)*));
            }else{
                eprintln!("{} {}", "!!!".bright_red().bold(), format!($($arg)*));
            }
        };
    }

    panic_log!("程序发生了无法挽回的异常 (Panic)，即将退出");
    panic_log!("如果你想要报告这个问题，请保留以下信息：");

    if let Some(location) = panic_info.location() {
        panic_log!(
            "Panic 发生在：{}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        panic_log!("无法获取 Panic 位置");
    }

    if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
        panic_log!("Panic 信息：{}", message);
    } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
        panic_log!("Panic 信息：{}", message);
    } else {
        panic_log!("无法获取 Panic 信息");
    }
    panic_log!("详见：https://docs.tuack-ng.ink/app/faq/panic.html");
}

pub(crate) fn init_log(verbose: &bool) -> Result<MultiProgress> {
    let format = if *verbose {
        "{d(%Y-%m-%d %H:%M:%S)} | {h({l})} | {t} | {m}{n}"
    } else {
        "{h({l})} | {m}{n}"
    };

    let loglevel = if *verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Warn
    };

    // RPC 模式：日志与进度条均改道为通知（否则会污染 stdout 的 JSON 流）
    if crate::rpc::is_enabled() {
        let config = Config::builder()
            .appender(Appender::builder().build("rpc", Box::new(RpcLogAppender)))
            .build(Root::builder().appender("rpc").build(loglevel))?;

        let logger: log4rs::Logger = Logger::new(config);
        let level = logger.max_log_level();
        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::term_like(Box::new(
            RpcTermLike::default(),
        )));
        LogWrapper::new(multi.clone(), logger).try_init().unwrap();
        log::set_max_level(level);

        return Ok(multi);
    }

    let stdout = ConsoleAppender::builder()
        .target(Target::Stderr)
        .encoder(Box::new(PatternEncoder::new(format)))
        .build();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(loglevel))?;

    let logger: log4rs::Logger = Logger::new(config);
    let level = logger.max_log_level();
    let multi = MultiProgress::new();
    LogWrapper::new(multi.clone(), logger).try_init().unwrap();
    log::set_max_level(level);

    Ok(multi)
}

/// RPC 模式：log 输出转为 output 通知
#[derive(Debug)]
struct RpcLogAppender;

impl log4rs::append::Append for RpcLogAppender {
    fn append(&self, record: &log::Record) -> anyhow::Result<()> {
        crate::rpc::emit_output("stderr", &format!("{} | {}", record.level(), record.args()));
        Ok(())
    }

    fn flush(&self) {}
}

/// RPC 模式：捕获 indicatif 的绘制输出并转为 progress 通知。
/// 多进度条场景仅保留最近绘制的一行（v1 简化）。
#[derive(Debug, Default)]
struct RpcTermLike {
    line: std::sync::Mutex<String>,
}

impl indicatif::TermLike for RpcTermLike {
    fn width(&self) -> u16 {
        120
    }

    fn move_cursor_up(&self, _n: usize) -> std::io::Result<()> {
        Ok(())
    }

    fn move_cursor_down(&self, _n: usize) -> std::io::Result<()> {
        Ok(())
    }

    fn move_cursor_right(&self, _n: usize) -> std::io::Result<()> {
        Ok(())
    }

    fn move_cursor_left(&self, _n: usize) -> std::io::Result<()> {
        Ok(())
    }

    fn write_line(&self, s: &str) -> std::io::Result<()> {
        *self.line.lock().unwrap() = s.trim_end_matches('\r').to_string();
        Ok(())
    }

    fn write_str(&self, s: &str) -> std::io::Result<()> {
        self.line.lock().unwrap().push_str(s);
        Ok(())
    }

    fn clear_line(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn flush(&self) -> std::io::Result<()> {
        let mut line = self.line.lock().unwrap();
        let text = line.trim().to_string();
        if !text.is_empty() {
            crate::rpc::emit_progress(&text);
            line.clear();
        }
        Ok(())
    }
}

pub(crate) fn init_context(multi: MultiProgress, migrating: bool, validating: bool) -> Result<()> {
    let home_dir = dirs::home_dir().context("无法获取 HOME 环境变量")?;

    debug!(
        "{:#?}",
        dirs::data_local_dir()
            .unwrap_or_else(|| home_dir.join(".local/share"))
            .join("tuack-ng")
    );

    let assets_dirs = vec![
        // 开发资源目录
        #[cfg(debug_assertions)]
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        // 用户目录
        dirs::data_local_dir()
            .unwrap_or_else(|| home_dir.join(".local/share"))
            .join("tuack-ng"),
        // 系统目录
        #[cfg(not(windows))]
        {
            // Nix 下，使用相对路径探测
            #[cfg(feature = "nix")]
            let path = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent()?.parent()?.join("share/tuack-ng").into())
                .context("找不到资源")?;

            #[cfg(not(feature = "nix"))]
            let path = PathBuf::from("/usr/share/tuack-ng/");

            path
        },
        #[cfg(windows)]
        {
            use std::env;
            let exe_path = env::current_exe().expect("Failed to get executable path");
            exe_path.parent().unwrap().join("assets")
        },
    ];

    let mut ctx = if migrating {
        LoadContext::new_force_migrate()
    } else {
        LoadContext::new()
    };

    let config = match load_config(&mut ctx, Path::new(".")) {
        Ok(res) => {
            if res.as_ref().is_some() {
                info!("当前路径：{:#?}", res.as_ref().unwrap().location);
                for (&from, message) in &ctx.migrated_notices {
                    let to = from + 1;
                    if message.is_empty() {
                        msg_warn!("来自从 {} 版本迁移到 {} 版本的信息", from, to);
                    } else {
                        msg_warn!("来自从 {} 版本迁移到 {} 版本的信息：{}", from, to, message);
                    }
                }
                if ctx.root.count() != 0 && !validating {
                    let err_count = ctx.root.count_errors();
                    let warn_count = ctx.root.count_warnings();
                    if warn_count > 0 {
                        msg_warn!(
                            "配置文件中发现了 {} 个警告。使用 `tuack-ng doc validate` 查看。",
                            warn_count
                        );
                    }
                    if err_count > 0 {
                        msg_error!("配置文件中发现了 {} 个错误：", err_count);
                        msg!("{}", ctx.render_errors_tree());
                    }
                }
            }
            res
        }
        Err(e) => {
            msg_warn!("配置文件解析失败，可能导致问题：{}", e);
            None
        }
    };

    let langs = assets_dirs
        .iter()
        .find_map(|dir| {
            dir.join("langs.json")
                .exists()
                .then(|| dir.join("langs.json"))
        })
        .unwrap_or_else(|| assets_dirs[0].join("langs.json"));

    let langs_content = fs::read_to_string(langs).unwrap();

    let languages = serde_json::from_str(&langs_content)?;

    context::setup_context(context::Context {
        assets_dirs,
        multiprogress: multi,
        config,
        loadctx: ctx,
        languages,
    })?;
    Ok(())
}

pub fn init(verbose: &bool, cli: &crate::Cli) -> Result<()> {
    if !DEBUG {
        let verbose_value = *verbose;
        panic::set_hook(Box::new(move |panic_info| {
            custom_panic_handler(panic_info, verbose_value);
        }));
    }
    let multi = init_log(verbose)?;
    // 生成补全文件时，有可能还没有全局配置文件亦或者不合法，所以可能会失败
    // 因此，跳过初始化逻辑
    if !matches!(cli.command, crate::Commands::Gen(ref args)
       if matches!(args.target, crate::generate::Targets::Complete(_)))
    {
        let migrating = if matches!(cli.command, crate::Commands::Conf(ref args)
       if matches!(args.target, crate::conf::Targets::Migrate))
        {
            true
        } else {
            false
        };
        let validating = if matches!(cli.command, crate::Commands::Doc(ref args)
       if matches!(args.target, crate::doc::Targets::Validate))
        {
            true
        } else {
            false
        };

        init_context(multi, migrating, validating)?;
    }
    Ok(())
}
