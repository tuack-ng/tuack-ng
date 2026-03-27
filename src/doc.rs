use crate::doc::format::FormatArgs;
use crate::prelude::*;
use check::CheckArgs;
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
    Format(FormatArgs),
    #[command(version)]
    /// 检查
    Check(CheckArgs),
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
        Targets::Format(args) => format::main(args)?,
        Targets::Check(args) => check::main(args)?,
    }

    Ok(())
}
