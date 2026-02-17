use crate::prelude::*;
use clap::Args;
use clap::ValueEnum;

mod lemon;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Target {
    Lemon,
}

#[derive(Args, Debug)]
#[command(version)]
pub struct DumpArgs {
    /// 渲染目标模板
    #[arg(required = true)]
    pub target: Target,
}

pub fn dump_main(day: &ContestDayConfig, target: Target) -> Result<()> {
    let dump_dir = day.path.join("dump");
    if !dump_dir.exists() {
        fs::create_dir(dump_dir)?;
    }

    match target {
        Target::Lemon => lemon::main(day),
    }
}

pub fn main(args: DumpArgs) -> Result<()> {
    if get_context().config.is_none() {
        bail!("没有有效的配置文件");
    }
    let config = get_context().config.clone().unwrap();
    match config.1 {
        CurrentLocation::None => bail!("此命令必须在工程下执行"),
        CurrentLocation::Problem(_, _) => bail!("此命令不能在题目下执行"),
        CurrentLocation::Day(day) => {
            dump_main(config.0.subconfig.get(&day).unwrap(), args.target)?;
        }
        CurrentLocation::Root => {
            for (_, day_config) in config.0.subconfig {
                dump_main(&day_config, args.target)?;
            }
        }
    }

    match args.target {
        Target::Lemon => (),
    }
    Ok(())
}
