use std::process::{Command as StdCommand, Stdio};
use tempfile::TempDir;

use crate::prelude::*;
use async_trait::async_trait;
use tokio::process::Command;
use tuack_lib::data::AsyncReader;
use tuack_lib::utils::testlib::{Arg, Generator};

pub struct CppGenerator {
    tmp_dir: TempDir,
    source: PathBuf,
    compile_args: String,
    binary_path: Option<PathBuf>,
    dependencies: IndexMap<String, Vec<u8>>,
}

impl CppGenerator {
    pub fn new(
        source: impl Into<PathBuf>,
        compile_args: &IndexMap<String, String>,
        dependencies: IndexMap<String, Vec<u8>>,
    ) -> Result<Self> {
        let source = source.into();
        let tmp_dir = TempDir::with_prefix("tuack-ng-generator-")?;
        let ext = source
            .extension()
            .context("没有后缀名")?
            .to_string_lossy()
            .into_owned();
        Ok(CppGenerator {
            tmp_dir,
            source,
            compile_args: compile_args
                .get(&ext)
                .cloned()
                .unwrap_or_else(|| "-O2 -std=c++17".to_string()),
            binary_path: None,
            dependencies,
        })
    }
}

#[async_trait]
impl Generator for CppGenerator {
    fn prepare(&mut self) -> Result<()> {
        if !self.tmp_dir.path().exists() {
            fs::create_dir_all(self.tmp_dir.path())?;
        }

        let source_target = self
            .tmp_dir
            .path()
            .join(self.source.file_name().context("无效的源文件名")?);
        fs::copy(&self.source, &source_target)?;

        for (name, content) in &self.dependencies {
            let target = self.tmp_dir.path().join(name);
            fs::write(&target, content)?;
        }

        let binary_path = self
            .tmp_dir
            .path()
            .join("gen")
            .with_extension(std::env::consts::EXE_EXTENSION);
        let mut cmd = StdCommand::new("g++");
        cmd.arg("-o").arg(&binary_path).arg(&source_target);

        let parsed = shellwords::split(&self.compile_args)?;
        if !parsed.is_empty() {
            cmd.args(&parsed);
        }

        let output = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output()?;
        if !output.status.success() {
            bail!(
                "生成器编译错误：{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fs::remove_file(&source_target)?;
        self.binary_path = Some(binary_path);
        Ok(())
    }

    async fn run(&self, args: IndexMap<String, Arg>, seed: u64) -> Result<Box<dyn AsyncReader>> {
        let binary = self
            .binary_path
            .as_ref()
            .context("生成器未编译，请先调用 prepare()")?;

        let mut cmd_args: Vec<String> = Vec::new();

        for (key, value) in args {
            let val_str = match value {
                Arg::Integer(v) => v.to_string(),
                Arg::Float(v) => v.to_string(),
                Arg::Str(v) => v,
                Arg::Bool(true) => "true".to_string(),
                Arg::Bool(false) => "false".to_string(),
            };
            cmd_args.push(format!("-{}={}", key, val_str));
        }

        cmd_args.push(format!("-seed={}", seed).to_string());

        // stdout/stderr 重定向到临时文件，避免整块读入内存
        let out_path = self.tmp_dir.path().join(format!("gen-{seed}.out"));
        let err_path = self.tmp_dir.path().join(format!("gen-{seed}.err"));
        let out_file = std::fs::File::create(&out_path)?;
        let err_file = std::fs::File::create(&err_path)?;

        let status = Command::new(binary)
            .args(&cmd_args)
            .stdout(Stdio::from(out_file))
            .stderr(Stdio::from(err_file))
            .status()
            .await?;

        if !status.success() {
            let err = tokio::fs::read_to_string(&err_path)
                .await
                .unwrap_or_default();
            bail!("生成器运行失败：{}", err);
        }

        let f = tokio::fs::File::open(&out_path).await?;
        Ok(Box::new(f))
    }
}
