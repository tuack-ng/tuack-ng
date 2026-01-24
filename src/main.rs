use crate::conf::ConfArgs;
use crate::fun::Philia093Args;
use crate::generate::GenArgs;
use crate::ren::RenArgs;
use crate::test::TestArgs;
use clap::ArgAction;
use clap::{Parser, Subcommand};
use clap_i18n_richformatter::clap_i18n;
use log::info;

mod conf;
mod config;
mod context;
mod fun;
mod generate;
mod init;
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
    /// 使用题解代码测试
    Conf(ConfArgs),
    /// see you *tomorrow*
    #[command(hide = true, name = "PhiLia093")]
    Philia093(Philia093Args),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse_i18n_or_exit();

    init::init(&(cli.verbose >= 1))?;

    info!("booting up");

    let result = match cli.command {
        Commands::Ren(args) => ren::main(args),
        Commands::Gen(args) => generate::main(args),
        Commands::Test(args) => test::main(args),
        Commands::Philia093(args) => fun::main(args, &cli.verbose),
        Commands::Conf(args) => conf::main(args),
    };

    if let Err(e) = result {
        log::error!("程序执行出错: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
