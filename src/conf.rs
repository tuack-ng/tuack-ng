// TODO: 搁置，先去重构

use crate::{
    Subcommand,
    context::{CurrentLocation, get_context},
};
use clap::Args;

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct ConfValuesArgs {
    /// 值
    #[arg(required = true)]
    name: Vec<String>,
}

#[derive(Subcommand, Debug)]
#[command(infer_subcommands = false)]
enum Targets {
    // /// 生成竞赛文件夹
    // #[command(version, aliases = ["n"])]
    // Contest(),
    // /// 生成竞赛日文件夹
    // #[command(version, alias = "d")]
    // Day(GenStatementArgs),
    // /// 生成题目文件夹
    // #[command(version, aliases = ["p", "prob"])]
    // Problem(GenStatementArgs),

    // /// 自动检测数据
    // #[command(version, alias = "t")]
    // Data(GenConfirmArgs),
    // /// 自动检测样例
    // #[command(version, alias = "s")]
    // Samples(GenConfirmArgs),
    // /// 自动检测题解
    // #[command(version, alias = "c")]
    // Code(GenConfirmArgs),
    // /// 自动检测所有
    // #[command(version, alias = "a")]
    // All(GenConfirmArgs),
    /// 设置标题
    #[command(version)]
    Title(ConfValuesArgs),
    #[command(version)]
    Time(ConfValuesArgs),
    #[command(version)]
    Length(ConfValuesArgs),
    #[command(version)]
    Conf(ConfValuesArgs),
}

#[derive(Args, Debug)]
#[command(version)]
pub struct ConfArgs {
    /// 目标对象
    #[command(subcommand)]
    target: Targets,
}

fn conf_title(args: &ConfValuesArgs) -> Result<(), Box<dyn std::error::Error>> {
    match get_context().config.as_ref().ok_or("没有找到有效的工程")?.1 {
        CurrentLocation::Problem(_, _) => Err("本命令不支持设置单个题目标题")?,
        CurrentLocation::Day(ref day) => {
            println!("Setting title for day {}: {}", day, args.name.join(" "));
        }
        _ => Err("没有找到有效的配置文件")?,
    }
    Ok(())
}

pub fn main(args: ConfArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Add your configuration logic here

    match args.target {
        Targets::Title(conf_args) => {
            conf_title(&conf_args)?;
        }
        Targets::Length(conf_args) => {
            for name in conf_args.name {
                println!("Setting title to: {}", name);
            }
        }
        Targets::Time(conf_args) => {
            for name in conf_args.name {
                println!("Setting time to: {}", name);
            }
        }
        Targets::Conf(conf_args) => {
            for name in conf_args.name {
                println!("Setting conf to: {}", name);
            }
        }
    }

    Ok(())
}
