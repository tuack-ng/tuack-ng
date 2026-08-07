use std::fs::File;
use std::process::{Command, Stdio};
use tempfile::{NamedTempFile, TempDir};

use crate::prelude::*;
use crate::tuack_lib::utils::testlib::{Validator, ValidatorResult};

pub struct CppValidator {
    tmp_dir: TempDir,
    source: PathBuf,
    compile_args: String,
    program_name: String,
    binary_path: Option<PathBuf>,
    dependencies: IndexMap<String, Vec<u8>>,
}

impl CppValidator {
    pub fn new(
        source: impl Into<PathBuf>,
        compile_args: &IndexMap<String, String>,
        program_name: impl Into<String>,
        dependencies: IndexMap<String, Vec<u8>>,
    ) -> Result<Self> {
        let source = source.into();
        let program_name = program_name.into();
        let tmp_dir = TempDir::with_prefix("tuack-ng-validator-")?;
        let ext = source
            .extension()
            .context("没有后缀名")?
            .to_string_lossy()
            .into_owned();
        Ok(CppValidator {
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

impl Validator for CppValidator {
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
            .join("val")
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
                "Validator 编译错误：{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fs::remove_file(&source_target)?;
        self.binary_path = Some(binary_path);
        Ok(())
    }

    fn validate(&self, input: &[u8]) -> Result<ValidatorResult> {
        let binary = self.binary_path.as_ref().context("Validator 未编译")?;

        let input_file = NamedTempFile::with_prefix("tuack-ng-validator-in-")?;
        fs::write(&input_file, input)?;
        let stdin_file = File::open(input_file.path())?;

        let stderr_file = NamedTempFile::with_prefix("tuack-ng-validator-err-")?;
        let stderr_f = File::create(stderr_file.path())?;

        let status = Command::new(binary)
            .stdin(Stdio::from(stdin_file))
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr_f))
            .status()?;

        let message = fs::read_to_string(stderr_file.path()).unwrap_or_default();

        Ok(if status.success() {
            ValidatorResult::Ok
        } else {
            ValidatorResult::Invalid(message.trim().to_string())
        })
    }
}
