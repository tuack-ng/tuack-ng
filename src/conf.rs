use crate::config::ContestDayConfig;
use crate::config::{CONFIG_FILE_NAME, save_day_config};
use std::fs;

use crate::config::ProblemConfig;
use chrono::Datelike;
use chrono::Timelike;
use chrono::{Duration, NaiveDateTime};

fn add_minutes(time: [u32; 6], minutes: i64) -> [u32; 6] {
    let dt = NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(time[0] as i32, time[1], time[2]).unwrap(),
        chrono::NaiveTime::from_hms_opt(time[3], time[4], time[5]).unwrap(),
    );

    let new_dt = dt + Duration::minutes(minutes);

    [
        new_dt.year() as u32,
        new_dt.month(),
        new_dt.day(),
        new_dt.hour(),
        new_dt.minute(),
        new_dt.second(),
    ]
}

use crate::{
    Subcommand,
    config::save_problem_config,
    context::{CurrentLocation, get_context},
};
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
enum Targets {
    /// 设置标题
    #[command(version)]
    Title(ConfValuesArgs),
    /// 设置时间限制
    #[command(version)]
    Time(ConfValuesArgs),
    #[command(version)]
    Length(ConfValuesArgs),
    #[command(version)]
    Conf(ConfCustomArgs),
}

#[derive(Args, Debug)]
#[command(version)]
pub struct ConfArgs {
    /// 目标对象
    #[command(subcommand)]
    target: Targets,
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
fn conf_title(args: &ConfValuesArgs) -> Result<(), Box<dyn std::error::Error>> {
    match get_context().config.as_ref().ok_or("没有找到有效的工程")?.1 {
        CurrentLocation::Problem(_, _) => Err("本命令不支持设置单个题目标题".into()),
        CurrentLocation::Day(ref day) => {
            let mut day_config = get_context()
                .config
                .as_ref()
                .ok_or("没有找到有效的工程")?
                .0
                .subconfig
                .get(day)
                .unwrap()
                .clone();
            if args.value.len() != day_config.subconfig.len() {
                return Err("提供的标题数量与题目数量不匹配".into());
            }
            for (i, (_prob_name, prob_config)) in day_config.subconfig.iter_mut().enumerate() {
                prob_config.title = args.value[i].clone();
                let conf_str = save_problem_config(prob_config)?;
                fs::write(prob_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::Root => {
            let mut config = get_context()
                .config
                .as_ref()
                .ok_or("没有找到有效的工程")?
                .0
                .clone();
            if args.value.len() != config.subconfig.len() {
                return Err("提供的标题数量与题目数量不匹配".into());
            }
            for (i, (_day_name, day_config)) in config.subconfig.iter_mut().enumerate() {
                day_config.title = args.value[i].clone();
                let conf_str = save_day_config(day_config)?;
                fs::write(day_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::None => Err("没有找到有效的配置文件".into()),
    }
}

fn conf_time(args: &ConfValuesArgs) -> Result<(), Box<dyn std::error::Error>> {
    match get_context().config.as_ref().ok_or("没有找到有效的工程")?.1 {
        CurrentLocation::Problem(_, _) => Err("本命令不支持设置单个题目时间限制".into()),
        CurrentLocation::Day(ref day) => {
            let mut day_config = get_context()
                .config
                .as_ref()
                .ok_or("没有找到有效的工程")?
                .0
                .subconfig
                .get(day)
                .unwrap()
                .clone();
            if args.value.len() != day_config.subconfig.len() {
                return Err("提供的时间限制数量与题目数量不匹配".into());
            }
            for (i, (_prob_name, prob_config)) in day_config.subconfig.iter_mut().enumerate() {
                prob_config.time_limit = args.value[i].clone().parse()?;
                let conf_str = save_problem_config(prob_config)?;
                fs::write(prob_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::Root => Err("本命令不能为比赛日设置时间限制".into()),
        CurrentLocation::None => Err("没有找到有效的配置文件".into()),
    }
}

fn conf_length(args: &ConfValuesArgs) -> Result<(), Box<dyn std::error::Error>> {
    match get_context().config.as_ref().ok_or("没有找到有效的工程")?.1 {
        CurrentLocation::Problem(_, _) => Err("本命令不支持设置单个题目时间限制".into()),
        CurrentLocation::Root => {
            let mut config = get_context()
                .config
                .as_ref()
                .ok_or("没有找到有效的工程")?
                .0
                .clone();
            if args.value.len() != config.subconfig.len() {
                return Err("提供的时间限制数量与题目数量不匹配".into());
            }
            for (i, (_day_name, day_config)) in config.subconfig.iter_mut().enumerate() {
                let minutes: f64 = args.value[i].clone().parse()?;
                let minutes = (minutes * 60.0) as i64;
                day_config.end_time = add_minutes(day_config.start_time, minutes);
                let conf_str = save_day_config(day_config)?;
                fs::write(day_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::Day(_) => Err("本命令不能为比赛日设置时间限制".into()),
        CurrentLocation::None => Err("没有找到有效的配置文件".into()),
    }
}

fn conf_custom(args: &ConfCustomArgs) -> Result<(), Box<dyn std::error::Error>> {
    match get_context().config.as_ref().ok_or("没有找到有效的工程")?.1 {
        CurrentLocation::Problem(_, _) => Err("本命令不支持设置单个题目标题".into()),
        CurrentLocation::Day(ref day) => {
            let mut day_config = get_context()
                .config
                .as_ref()
                .ok_or("没有找到有效的工程")?
                .0
                .subconfig
                .get(day)
                .unwrap()
                .clone();
            if args.value.len() != day_config.subconfig.len() {
                return Err("提供的标题数量与题目数量不匹配".into());
            }
            for (i, (_prob_name, prob_config)) in day_config.subconfig.iter_mut().enumerate() {
                let mut json = serde_json::to_value(&prob_config).unwrap();
                let value = serde_json::from_str::<serde_json::Value>(&args.value[i])
                    .map_err(|e| format!("值解析失败：{e}"))?;
                json.as_object_mut()
                    .unwrap()
                    .insert(args.key.clone(), value);
                let updated_config = serde_json::from_value::<ProblemConfig>(json)
                    .map_err(|e| format!("json 序列化失败，可能是因为提供了无效的值：{e}"))?;
                let conf_str = save_problem_config(&updated_config)?;
                fs::write(prob_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::Root => {
            let mut config = get_context()
                .config
                .as_ref()
                .ok_or("没有找到有效的工程")?
                .0
                .clone();
            if args.value.len() != config.subconfig.len() {
                return Err("提供的标题数量与题目数量不匹配".into());
            }
            for (i, (_day_name, day_config)) in config.subconfig.iter_mut().enumerate() {
                let mut json = serde_json::to_value(&day_config)?;
                let value = serde_json::from_str::<serde_json::Value>(&args.value[i])?;
                json.as_object_mut()
                    .unwrap()
                    .insert(args.key.clone(), value);
                let updated_config = serde_json::from_value::<ContestDayConfig>(json)?;
                let conf_str = save_day_config(&updated_config)?;
                fs::write(day_config.path.join(CONFIG_FILE_NAME), conf_str)?;
            }
            Ok(())
        }
        CurrentLocation::None => Err("没有找到有效的配置文件".into()),
    }
}

pub fn main(args: ConfArgs) -> Result<(), Box<dyn std::error::Error>> {
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
    }

    Ok(())
}
