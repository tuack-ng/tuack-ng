use crate::conf::ConfArgs;
use crate::dmk::DmkArgs;
use crate::dump::DumpArgs;
use crate::generate::GenArgs;
use crate::prelude::*;
use crate::ren::RenArgs;
use crate::test::TestArgs;
use clap::ArgAction;
use clap::{Parser, Subcommand};
use clap_i18n_richformatter::clap_i18n;

mod conf;
mod config;
mod context;
mod dmk;
mod dump;
mod generate;
mod init;
mod prelude;
mod ren;
mod test;
mod utils;

#[derive(Debug, Parser)]
#[clap_i18n]
#[command(version, about = "Tuack-NG", disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    #[arg(short, long, global = true, action = ArgAction::Count)]
    /// 详细模式
    verbose: u8,
}

#[derive(Subcommand, Debug)]
#[command(infer_subcommands = false)]
enum Commands {
    /// 渲染题面
    Ren(RenArgs),
    /// 生成工程文件夹
    Gen(GenArgs),
    /// 使用题解代码测试
    Test(TestArgs),
    /// 批量修改配置文件
    Conf(ConfArgs),
    /// 生成数据
    Dmk(DmkArgs),
    /// 导出到评测系统
    Dump(DumpArgs),
}

fn tuack_ng(cli: Cli) -> Result<()> {
    // 生成补全文件时，有可能还没有全局配置文件亦或者不合法，所以可能会失败
    // 因此，跳过初始化逻辑
    if !matches!(cli.command, Commands::Gen(ref args)
       if matches!(args.target, crate::generate::Targets::Complete(_)))
    {
        init::init(&(cli.verbose >= 1))?;
        info!("booting up");
    }

    match cli.command {
        Commands::Ren(args) => ren::main(args),
        Commands::Gen(args) => generate::main(args),
        Commands::Test(args) => test::main(args),
        Commands::Conf(args) => conf::main(args),
        Commands::Dmk(args) => dmk::main(args),
        Commands::Dump(args) => dump::main(args),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse_i18n_or_exit();

    let result = tuack_ng(cli);

    if cfg!(debug_assertions) {
        result?;
    } else if let Err(e) = result {
        if log::max_level() == log::LevelFilter::Off {
            eprintln!("程序执行出错: {:#}", e);
        } else {
            log::error!("程序执行出错: {:#}", e);
        }
        std::process::exit(1);
    }
    Ok(())
}
