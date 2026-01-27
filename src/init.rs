use crate::prelude::*;
use log::LevelFilter;
use log4rs::append::console::Target;
use log4rs::{
    Logger,
    append::console::ConsoleAppender,
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use std::fs;
use std::path::PathBuf;
use std::{env, path::Path};

use crate::{config::load_config, context};
use chrono::Local;
use colored::Colorize;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use std::panic::{self, PanicHookInfo};

#[cfg(debug_assertions)]
const DEBUG: bool = true;
#[cfg(not(debug_assertions))]
const DEBUG: bool = false;

fn custom_panic_handler(panic_info: &PanicHookInfo, verbose: bool) {
    let prefix = || "PANIC".bright_red().bold().on_black();

    macro_rules! panic_log {
        ($($arg:tt)*) => {
            if verbose {
                eprintln!("{} | {} | {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                prefix(), format!($($arg)*));
            }else{
                eprintln!("{} | {}", prefix(), format!($($arg)*));
            }
        };
    }

    panic_log!("程序发生了无法挽回的异常，即将退出");
    panic_log!("如果你想要报告这个问题，请保留以下信息:");

    if let Some(location) = panic_info.location() {
        panic_log!(
            "Panic 发生在: {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        panic_log!("无法获取 Panic 位置");
    }

    if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
        panic_log!("Panic 信息: {}", message);
    } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
        panic_log!("Panic 信息: {}", message);
    } else {
        panic_log!("无法获取 Panic 信息");
    }
    panic_log!("详见: https://docs.tuack-ng.ink/contributing/panic.html");
}

fn init_log(verbose: &bool) -> Result<MultiProgress> {
    let format = if DEBUG || *verbose {
        "{d(%Y-%m-%d %H:%M:%S)} | {h({l})} | {t} | {m}{n}"
    } else {
        "{h({l})} | {m}{n}"
    };

    let stdout = ConsoleAppender::builder()
        .target(Target::Stderr)
        .encoder(Box::new(PatternEncoder::new(format)))
        .build();

    let loglevel = if DEBUG || *verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Warn
    };

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

fn init_context(multi: MultiProgress) -> Result<()> {
    let home_dir = env::var("HOME")
        .inspect_err(|e| log::error!("无法获取 HOME 环境变量: {}", e))
        .context("无法获取 HOME 环境变量")?;

    let assets_dirs = vec![
        #[cfg(debug_assertions)]
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        PathBuf::from(&home_dir).join(".local/share/tuack-ng/"),
        PathBuf::from("/usr/share/tuack-ng/"),
    ];

    let config = match load_config(Path::new(".")) {
        Ok(res) => {
            if res.as_ref().is_some() {
                info!("当前路径: {:#?}", res.as_ref().unwrap().1);
            }
            res
        }
        Err(e) => {
            warn!("配置文件解析失败，可能导致问题: {}", e);
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
        .unwrap_or_else(|| get_context().assets_dirs[0].join("langs.json"));

    let langs_content = fs::read_to_string(langs).unwrap();

    let languages = serde_json::from_str(&langs_content)?;

    context::setup_context(context::Context {
        assets_dirs,
        multiprogress: multi,
        config,
        languages,
    })?;
    Ok(())
}

pub fn init(verbose: &bool) -> Result<()> {
    let multi = init_log(verbose)?;
    init_context(multi)?;
    if !DEBUG {
        let verbose_value = *verbose;
        panic::set_hook(Box::new(move |panic_info| {
            custom_panic_handler(panic_info, verbose_value);
        }));
    }
    Ok(())
}
