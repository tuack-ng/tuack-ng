use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

use crate::prelude::*;
use crate::tuack_lib::utils::testlib::Checker;
use crate::utils::checkers::helper::{JudgeResult, parse_result};

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

impl Checker for PrebuiltChecker {
    fn prepare(&mut self) -> Result<()> {
        if !self.binary.exists() {
            bail!("预编译 Checker 不存在：{}", self.binary.display());
        }
        Ok(())
    }

    fn validate(
        &self,
        input: &Path,
        output: &[u8],
        answer: &Path,
    ) -> Result<(JudgeResult, String)> {
        let output_path = NamedTempFile::with_prefix("tuack-ng-checker-out-")?;
        fs::write(&output_path, output)?;

        let res_path = NamedTempFile::with_prefix("tuack-ng-checker-res-")?;

        let _status = Command::new(&self.binary)
            .arg(input)
            .arg(output_path.path())
            .arg(answer)
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
