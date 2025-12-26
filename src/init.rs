use log::LevelFilter;
use log4rs::{
    Logger,
    append::console::ConsoleAppender,
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use std::env;
use std::path::PathBuf;

use crate::context;
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
}

fn init_log(verbose: &bool) -> Result<MultiProgress, Box<dyn std::error::Error>> {
    let format = if DEBUG || *verbose {
        "{d(%Y-%m-%d %H:%M:%S)} | {h({l})} | {t} | {m}{n}"
    } else {
        "{h({l})} | {m}{n}"
    };

    let stdout = ConsoleAppender::builder()
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

fn init_context(multi: MultiProgress) -> Result<(), Box<dyn std::error::Error>> {
    let home_dir = env::var("HOME").map_err(|e| {
        log::error!("无法获取 HOME 环境变量: {}", e);
        e
    })?;

    let template_dirs = vec![
        #[cfg(debug_assertions)]
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates"),
        PathBuf::from(&home_dir).join(".local/share/tuack-ng/templates"),
        PathBuf::from("/usr/share/tuack-ng/templates"),
    ];
    let scaffold_dirs = vec![
        #[cfg(debug_assertions)]
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scaffold"),
        PathBuf::from(&home_dir).join(".local/share/tuack-ng/scaffold"),
        PathBuf::from("/usr/share/tuack-ng/scaffold"),
    ];

    context::setup_context(context::Context {
        template_dirs: template_dirs,
        scaffold_dirs: scaffold_dirs,
        multiprogress: multi,
    })?;
    Ok(())
}

pub fn init(verbose: &bool) -> Result<(), Box<dyn std::error::Error>> {
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
