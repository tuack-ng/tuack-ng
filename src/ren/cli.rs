use clap::Args;
use log::{debug, error, info};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::{
    config::{ContestDayConfig, load_config},
    context,
};

use super::renderer::render_day;

#[derive(Args, Debug)]
#[command(version)]
pub struct RenArgs {
    /// 渲染目标模板
    #[arg(required = true)]
    pub target: String,

    /// 要渲染的天的名称（可选，如果不指定则渲染所有天）
    #[arg(short, long)]
    pub day: Option<String>,

    /// 保留临时目录用于调试
    #[arg(long)]
    pub keep_tmp: bool,
}

pub fn main(args: RenArgs) -> Result<(), Box<dyn std::error::Error>> {
    debug!(
        "当前目录: {}",
        Path::new(".").canonicalize()?.to_string_lossy()
    );
    let config = load_config(Path::new("."))?;

    let template_dir = context::get_context().template_dirs.iter().find(|dir| {
        let subdir = dir.join(&args.target);
        subdir.exists() && subdir.is_dir()
    });

    let template_dir = match template_dir {
        Some(dir) => {
            info!("找到模板目录: {}", dir.join(&args.target).to_string_lossy());
            dir.join(&args.target)
        }
        None => {
            error!("没有找到模板 {}", args.target);
            return Err(format!("致命错误: 没有找到模板 {}", args.target).into());
        }
    };

    debug!("检查Typst编译环境");
    let typst_check = Command::new("typst").arg("--version").output();

    match typst_check {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                debug!("Typst 版本: {}", version.trim());
            } else {
                error!("Typst 命令执行失败，请检查是否已安装");
                return Err("Typst 命令执行失败，请检查是否已安装".into());
            }
        }
        Err(_) => {
            return Err("未找到 typst 命令，请确保已安装并添加到PATH".into());
        }
    }

    let template_required_files = ["main.typ", "utils.typ"];
    for file in template_required_files {
        if !template_dir.join(file).exists() {
            error!("模板缺少必要文件: {}", file);
            return Err(format!("模板缺少必要文件: {}", file).into());
        }
        info!("文件存在: {}", file);
    }

    let statements_dir = config.path.join("statements/");
    info!("{}", &statements_dir.to_string_lossy());
    if !statements_dir.exists() {
        fs::create_dir(&statements_dir)?;
        info!("创建题面输出目录: {}", statements_dir.display());
    }

    // 过滤要渲染的天
    let days_to_render: Vec<&ContestDayConfig> = if let Some(day_name) = &args.day {
        match config.subconfig.iter().find(|d| d.name == *day_name) {
            Some(day) => {
                info!("渲染指定天: {}", day_name);
                vec![day]
            }
            None => {
                error!("未找到天: {}", day_name);
                return Err(format!("未找到天: {}", day_name).into());
            }
        }
    } else {
        info!("渲染所有天（共{}个）", config.subconfig.len());
        config.subconfig.iter().collect()
    };

    for day in days_to_render {
        info!("开始渲染天: {}", day.name);
        render_day(&config, day, &template_dir, &statements_dir, &args)?;
    }

    info!("所有天的题面渲染完成！");
    Ok(())
}
