use crate::prelude::*;
use clap::Args;
use clap::Subcommand;

pub mod check;
pub mod format;
pub mod rules;

#[derive(Debug, Clone, Subcommand)]
#[command(version)]
#[command(infer_subcommands = false)]
pub enum Targets {
    #[command(version)]
    /// 格式化
    Format,
    #[command(version)]
    /// 检查
    Check,
}

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct DocArgs {
    /// 生成的对象
    #[command(subcommand)]
    pub target: Targets,
}

pub fn main(args: DocArgs) -> Result<()> {
    match args.target {
        Targets::Format => format::main()?,
        Targets::Check => check::main()?,
    }

    Ok(())
}
