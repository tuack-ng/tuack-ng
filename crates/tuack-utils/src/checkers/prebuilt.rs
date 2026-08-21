use std::process::{Command, Stdio};

use crate::checkers::helper::{JudgeResult, parse_result, write_temp};
use crate::prelude::*;
use async_trait::async_trait;
use tuack_lib::data::AsyncReader;
use tuack_lib::utils::testlib::Checker;

/// 使用预编译的 Checker（如 `assets/checkers/normal`）
pub struct PrebuiltChecker {
    binary: PathBuf,
}

impl PrebuiltChecker {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        PrebuiltChecker {
            binary: binary.into(),
        }
    }
}

#[async_trait]
impl Checker for PrebuiltChecker {
    fn prepare(&mut self) -> Result<()> {
        if !self.binary.exists() {
            bail!("预编译 Checker 不存在：{}", self.binary.display());
        }
        Ok(())
    }

    async fn validate(
        &self,
        input: &mut dyn AsyncReader,
        output: &mut dyn AsyncReader,
        answer: &mut dyn AsyncReader,
    ) -> Result<(JudgeResult, String)> {
        let input_path = write_temp(input, "tuack-ng-checker-in-").await?;
        let output_path = write_temp(output, "tuack-ng-checker-out-").await?;
        let answer_path = write_temp(answer, "tuack-ng-checker-ans-").await?;

        let res_path = tempfile::NamedTempFile::with_prefix("tuack-ng-checker-res-")?;

        let _status = Command::new(&self.binary)
            .arg(input_path.path())
            .arg(output_path.path())
            .arg(answer_path.path())
            .arg(res_path.path())
            .arg("-appes")
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()?;

        let res_content = fs::read_to_string(res_path.path()).context("Checker 未生成报告文件")?;
        let (result, message) =
            parse_result(&res_content).map_err(|e| anyhow!("无法解析 Checker 结果：{}", e))?;

        Ok((result, message))
    }
}
