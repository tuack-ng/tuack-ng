use crate::prelude::*;
use chrono::Datelike;
use chrono::Timelike;
use chrono::{Duration, NaiveDateTime};
use tuack_config::{CONFIG_FILE_NAME, save_config};

fn add_minutes(time: [u32; 6], minutes: i64) -> Result<[u32; 6]> {
    let dt = NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(time[0] as i32, time[1], time[2])
            .context("date invalid")?,
        chrono::NaiveTime::from_hms_opt(time[3], time[4], time[5]).context("time invalid")?,
    );

    let new_dt = dt + Duration::minutes(minutes);

    Ok([
        new_dt.year() as u32,
        new_dt.month(),
        new_dt.day(),
        new_dt.hour(),
        new_dt.minute(),
        new_dt.second(),
    ])
}

use crate::{Subcommand, context::gctx};
use clap::Args;

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct ConfValuesArgs {
    /// 值
    #[arg(required = true)]
    value: Vec<String>,
}

#[derive(Subcommand, Debug)]
#[command(infer_subcommands = false)]
pub enum Targets {
    /// 设置标题
    #[command(version)]
    Title(ConfValuesArgs),
    /// 设置时间限制
    #[command(version)]
    Time(ConfValuesArgs),
    /// 设置比赛长度
    #[command(version)]
    Length(ConfValuesArgs),
    /// 设置任意字段
    #[command(version)]
    Conf(ConfCustomArgs),
    /// 迁移配置文件
    #[command(version)]
    Migrate,
}

#[derive(Args, Debug)]
#[command(version)]
pub struct ConfArgs {
    /// 目标对象
    #[command(subcommand)]
    pub target: Targets,
}

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct ConfCustomArgs {
    /// 键
    key: String,
    /// 值
    #[arg(required = true)]
    value: Vec<String>,
}
fn conf_title(args: &ConfValuesArgs) -> Result<()> {
    match gctx()
        .config
        .as_ref()
        .context("没有找到有效的工程")?
        .location
    {
        CurrentLocation::Problem(_, _) => bail!("本命令不支持设置单个题目标题"),
        CurrentLocation::Day(ref day) => {
            let mut day_config = gctx()
                .config
                .as_ref()
                .context("没有找到有效的工程")?
                .config
                .subconfig
                .get(day)
                .unwrap()
                .clone();
            if args.value.len() != day_config.subconfig.len() {
                bail!("提供的标题数量与题目数量不匹配");
            }
            for (i, (_prob_name, prob_config)) in day_config.subconfig.iter_mut().enumerate() {
                prob_config.title = args.value[i].clone();
                let conf_str = prob_config.save()?;
                fs::write(prob_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::Root => {
            let mut config = gctx()
                .config
                .as_ref()
                .context("没有找到有效的工程")?
                .config
                .clone();
            if args.value.len() != config.subconfig.len() {
                bail!("提供的标题数量与题目数量不匹配");
            }
            for (i, (_day_name, day_config)) in config.subconfig.iter_mut().enumerate() {
                day_config.title = args.value[i].clone();
                let conf_str = day_config.save()?;
                fs::write(day_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::None => bail!("没有找到有效的配置文件"),
    }
}

fn conf_time(args: &ConfValuesArgs) -> Result<()> {
    match gctx()
        .config
        .as_ref()
        .context("没有找到有效的工程")?
        .location
    {
        CurrentLocation::Problem(_, _) => bail!("本命令不支持设置单个题目时间限制"),
        CurrentLocation::Day(ref day) => {
            let mut day_config = gctx()
                .config
                .as_ref()
                .context("没有找到有效的工程")?
                .config
                .subconfig
                .get(day)
                .unwrap()
                .clone();
            if args.value.len() != day_config.subconfig.len() {
                bail!("提供的时间限制数量与题目数量不匹配");
            }
            for (i, (_prob_name, prob_config)) in day_config.subconfig.iter_mut().enumerate() {
                prob_config.time_limit = args.value[i].clone().parse()?;
                let conf_str = prob_config.save()?;
                fs::write(prob_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::Root => bail!("本命令不能为比赛日设置时间限制"),
        CurrentLocation::None => bail!("没有找到有效的配置文件"),
    }
}

fn conf_length(args: &ConfValuesArgs) -> Result<()> {
    match gctx()
        .config
        .as_ref()
        .context("没有找到有效的工程")?
        .location
    {
        CurrentLocation::Problem(_, _) => {
            bail!("本命令不能在题目目录使用，请在比赛根目录设置比赛长度")
        }
        CurrentLocation::Root => {
            let mut config = gctx()
                .config
                .as_ref()
                .context("没有找到有效的工程")?
                .config
                .clone();
            if args.value.len() != config.subconfig.len() {
                bail!("提供的比赛长度数量与比赛日数量不匹配");
            }
            for (i, (day_name, day_config)) in config.subconfig.iter_mut().enumerate() {
                let hours: f64 = args.value[i].clone().parse()?;
                let minutes = (hours * 60.0) as i64;
                if let Some(start_time) = day_config.start_time {
                    day_config.end_time = Some(add_minutes(start_time, minutes)?);
                } else {
                    msg_warn!("比赛日 {} 没有配置比赛开始时间，跳过", day_name);
                }
                let conf_str = day_config.save()?;
                fs::write(day_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::Day(_) => bail!("本命令不能在比赛日目录使用，请在比赛根目录设置比赛长度"),
        CurrentLocation::None => bail!("没有找到有效的配置文件"),
    }
}

fn conf_custom(args: &ConfCustomArgs) -> Result<()> {
    match gctx()
        .config
        .as_ref()
        .context("没有找到有效的工程")?
        .location
    {
        CurrentLocation::Problem(_, _) => bail!("本命令不能为单个题目设置任意字段"),
        CurrentLocation::Day(ref day) => {
            let mut day_config = gctx()
                .config
                .as_ref()
                .context("没有找到有效的工程")?
                .config
                .subconfig
                .get(day)
                .unwrap()
                .clone();
            if args.value.len() != day_config.subconfig.len() {
                bail!("提供的键值数量与题目数量不匹配");
            }
            for (i, (_prob_name, prob_config)) in day_config.subconfig.iter_mut().enumerate() {
                let mut json = serde_json::to_value(AsSerde::<ProblemConfig, FileView>::new(
                    prob_config.clone(),
                ))
                .unwrap();
                let value = serde_json::from_str::<serde_json::Value>(&args.value[i])
                    .context("值解析失败")?;
                json.as_object_mut()
                    .unwrap()
                    .insert(args.key.clone(), value);
                let updated_config =
                    serde_json::from_value::<AsSerde<ProblemConfig, FileView>>(json)
                        .context("json 序列化失败，可能是因为提供了无效的值")?
                        .into_inner();
                let conf_str = updated_config.save()?;
                fs::write(prob_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::Root => {
            let mut config = gctx()
                .config
                .as_ref()
                .context("没有找到有效的工程")?
                .config
                .clone();
            if args.value.len() != config.subconfig.len() {
                bail!("提供的键值数量与比赛日数量不匹配");
            }
            for (i, (_day_name, day_config)) in config.subconfig.iter_mut().enumerate() {
                let mut json = serde_json::to_value(AsSerde::<ContestDayConfig, FileView>::new(
                    day_config.clone(),
                ))?;
                let value = serde_json::from_str::<serde_json::Value>(&args.value[i])
                    .context("值解析失败")?;
                json.as_object_mut()
                    .unwrap()
                    .insert(args.key.clone(), value);
                let updated_config =
                    serde_json::from_value::<AsSerde<ContestDayConfig, FileView>>(json)
                        .context("json 序列化失败，可能是因为提供了无效的值")?
                        .into_inner();
                let conf_str = updated_config.save()?;
                fs::write(day_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::None => bail!("没有找到有效的配置文件"),
    }
}

fn conf_migrate() -> Result<()> {
    let config = gctx()
        .config
        .clone()
        .context("没有找到有效的配置文件")?
        .config;
    save_config(&config, &config.path)?;
    msg_info!("迁移完成！");
    Ok(())
}

pub fn main(args: ConfArgs) -> Result<()> {
    match args.target {
        Targets::Title(conf_args) => {
            conf_title(&conf_args)?;
        }
        Targets::Length(conf_args) => {
            conf_length(&conf_args)?;
        }
        Targets::Time(conf_args) => {
            conf_time(&conf_args)?;
        }
        Targets::Conf(conf_args) => {
            conf_custom(&conf_args)?;
        }
        Targets::Migrate => {
            conf_migrate()?;
        }
    }

    Ok(())
}
