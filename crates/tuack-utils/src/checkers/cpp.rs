use std::process::{Command, Stdio};
use tempfile::TempDir;

use crate::checkers::helper::{JudgeResult, parse_result, write_temp};
use crate::prelude::*;
use async_trait::async_trait;
use tuack_lib::data::AsyncReader;
use tuack_lib::utils::testlib::Checker;

pub struct CppChecker {
    tmp_dir: TempDir,
    source: PathBuf,
    compile_args: String,
    program_name: String,
    binary_path: Option<PathBuf>,
    dependencies: IndexMap<String, Vec<u8>>,
}

impl CppChecker {
    pub fn new(
        source: impl Into<PathBuf>,
        compile_args: &IndexMap<String, String>,
        program_name: impl Into<String>,
        dependencies: IndexMap<String, Vec<u8>>,
    ) -> Result<Self> {
        let source = source.into();
        let program_name = program_name.into();
        let tmp_dir = TempDir::with_prefix("tuack-ng-checker-")?;
        let ext = source
            .extension()
            .context("没有后缀名")?
            .to_string_lossy()
            .into_owned();
        Ok(CppChecker {
            tmp_dir,
            source,
            compile_args: compile_args
                .get(&ext)
                .cloned()
                .unwrap_or_else(|| "-O2 -std=c++17".to_string()),
            program_name,
            binary_path: None,
            dependencies,
        })
    }
}

#[async_trait]
impl Checker for CppChecker {
    fn prepare(&mut self) -> Result<()> {
        if !self.tmp_dir.path().exists() {
            fs::create_dir_all(self.tmp_dir.path())?;
        }

        let source_target = self
            .tmp_dir
            .path()
            .join(&self.program_name)
            .with_extension(self.source.extension().unwrap());
        fs::copy(&self.source, &source_target)?;

        for (name, content) in &self.dependencies {
            let target = self.tmp_dir.path().join(name);
            fs::write(&target, content)?;
        }

        let binary_path = self
            .tmp_dir
            .path()
            .join("chk")
            .with_extension(std::env::consts::EXE_EXTENSION);
        let mut cmd = Command::new("g++");
        cmd.arg("-o").arg(&binary_path).arg(&source_target);

        let parsed = shellwords::split(&self.compile_args)?;
        if !parsed.is_empty() {
            cmd.args(&parsed);
        }

        let output = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output()?;
        if !output.status.success() {
            bail!(
                "Checker 编译错误：{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fs::remove_file(&source_target)?;
        self.binary_path = Some(binary_path);
        Ok(())
    }

    async fn validate(
        &self,
        input: &mut dyn AsyncReader,
        output: &mut dyn AsyncReader,
        answer: &mut dyn AsyncReader,
    ) -> Result<(JudgeResult, String)> {
        let binary = self.binary_path.as_ref().context("Checker 未编译")?;

        let input_path = write_temp(input, "tuack-ng-checker-in-").await?;
        let output_path = write_temp(output, "tuack-ng-checker-out-").await?;
        let answer_path = write_temp(answer, "tuack-ng-checker-ans-").await?;

        let res_path = tempfile::NamedTempFile::with_prefix("tuack-ng-checker-res-")?;

        let _status = Command::new(binary)
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
