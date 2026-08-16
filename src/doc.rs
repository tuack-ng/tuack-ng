use crate::doc::format::FormatArgs;
use crate::prelude::*;
use check::CheckArgs;
use clap::Args;
use clap::Subcommand;

pub mod check;
pub mod format;
pub mod rules;
pub mod span;

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "snake_case"
)]
#[command(version)]
#[command(infer_subcommands = false)]
pub enum Targets {
    #[command(version)]
    /// 格式化
    Format(FormatArgs),
    #[command(version)]
    /// 检查
    Check(CheckArgs),
    #[command(version)]
    /// 查看配置文件错误
    Validate,
}

#[derive(Args, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
        Targets::Validate => msg!("{}", gctx().loadctx.render_tree()),
    }

    Ok(())
}
