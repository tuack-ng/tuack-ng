use crate::data::DmkData;
use crate::prelude::*;
use crate::utils::compiler::{IoMode, ResourceLimits, RunStatus, Runner};
use crate::utils::testlib::{Generator, Validator, ValidatorResult};

/// 数据生成参数（纯数据，对齐 `TaskParams`）。
#[derive(Debug, Clone)]
pub struct DmkParams {
    /// 题目名称（文件 IO 的输入输出文件名前缀）。
    pub problem_name: String,
    /// 是否使用文件 IO。
    pub file_io: bool,
}

/// 数据生成会话
pub struct DmkSession<'a> {
    /// 已 prepare 的标程运行器。
    runner: &'a mut dyn Runner,
    /// 已 prepare 的数据生成器。
    generator: &'a mut dyn Generator,
    /// 已 prepare 的输入校验器（可为空）。
    validator: Option<&'a dyn Validator>,
    /// 运行参数。
    params: DmkParams,
}

impl<'a> DmkSession<'a> {
    /// 创建会话。需保证 `runner` / `generator` / `validator` 已完成 prepare。
    pub fn new(
        runner: &'a mut dyn Runner,
        generator: &'a mut dyn Generator,
        validator: Option<&'a dyn Validator>,
        params: DmkParams,
    ) -> Self {
        Self {
            runner,
            generator,
            validator,
            params,
        }
    }

    /// 生成单点输入
    pub async fn gen_input(&self, item: &dyn DmkData, seed: u64) -> Result<()> {
        let stream = self.generator.run(item.args().clone(), seed).await?;
        item.write_input(stream).await?;

        let Some(validator) = self.validator else {
            return Ok(());
        };
        let mut input = item.input().await?;
        match validator.validate(&mut *input).await? {
            ValidatorResult::Ok => Ok(()),
            ValidatorResult::Invalid(message) => bail!("输入校验失败：{}", message),
        }
    }

    /// 用标程生成单点输出
    pub async fn gen_output(&mut self, item: &dyn DmkData) -> Result<()> {
        let input = item.input().await?;

        self.runner.set_input(input);
        self.runner.set_io_mode(if self.params.file_io {
            IoMode::File {
                input_name: format!("{}.in", self.params.problem_name),
                output_name: format!("{}.out", self.params.problem_name),
            }
        } else {
            IoMode::Stdio
        });
        self.runner.set_limits(ResourceLimits::unlimited());

        let result = self.runner.execute().await?;

        match result.status {
            RunStatus::Success => {}
            _ if !result.stderr.is_empty() => {
                bail!(
                    "标程运行失败\n标准错误输出：{}",
                    String::from_utf8_lossy(&result.stderr)
                );
            }
            _ => bail!("标程运行失败"),
        }

        let output = match result.output {
            Some(out) => out,
            None => bail!("标程未生成输出"),
        };
        item.write_output(output).await
    }
}
