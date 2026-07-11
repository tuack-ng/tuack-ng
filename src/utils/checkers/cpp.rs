use std::process::{Command, Stdio};
use tempfile::{NamedTempFile, TempDir};

use crate::prelude::*;
use crate::tuack_lib::utils::testlib::Checker;
use crate::utils::checkers::helper::{JudgeResult, parse_result};

pub struct CppChecker {
    tmp_dir: TempDir,
    source: PathBuf,
    compile_args: String,
    program_name: String,
    binary_path: Option<PathBuf>,
    dependencies: HashMap<String, Vec<u8>>,
}

impl CppChecker {
    pub fn new(
        source: impl Into<PathBuf>,
        compile_args: &HashMap<String, String>,
        program_name: impl Into<String>,
        dependencies: HashMap<String, Vec<u8>>,
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
                .unwrap_or_else(|| "-O2 -std=c++23".to_string()),
            program_name,
            binary_path: None,
            dependencies,
        })
    }
}

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

        let binary_path = self.tmp_dir.path().join("chk");
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

    fn validate(
        &self,
        input: &Path,
        output: &[u8],
        answer: &Path,
    ) -> Result<(JudgeResult, String)> {
        let binary = self.binary_path.as_ref().context("Checker 未编译")?;

        let output_path = NamedTempFile::with_prefix("tuack-ng-checker-out-")?;
        fs::write(&output_path, output)?;

        let res_path = NamedTempFile::with_prefix("tuack-ng-checker-res-")?;

        let _status = Command::new(binary)
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
