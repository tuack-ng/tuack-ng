use std::process::{Command, Stdio};
use tempfile::TempDir;

use crate::prelude::*;
use crate::tuack_lib::utils::testlib::{Arg, Generator};

pub struct CppGenerator {
    tmp_dir: TempDir,
    source: PathBuf,
    compile_args: String,
    program_name: String,
    binary_path: Option<PathBuf>,
    dependencies: HashMap<String, Vec<u8>>,
}

impl CppGenerator {
    pub fn new(
        source: impl Into<PathBuf>,
        compile_args: &HashMap<String, String>,
        program_name: impl Into<String>,
        dependencies: HashMap<String, Vec<u8>>,
    ) -> Result<Self> {
        let source = source.into();
        let program_name = program_name.into();
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
            program_name,
            binary_path: None,
            dependencies,
        })
    }
}

impl Generator for CppGenerator {
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

        let binary_path = self.tmp_dir.path().join("gen");
        let mut cmd = Command::new("g++");
        cmd.arg("-o")
            .arg(&binary_path)
            .arg(&source_target);

        let parsed = shellwords::split(&self.compile_args)?;
        if !parsed.is_empty() {
            cmd.args(&parsed);
        }

        let output = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output()?;
        if !output.status.success() {
            bail!("生成器编译错误：{}", String::from_utf8_lossy(&output.stderr));
        }

        fs::remove_file(&source_target)?;
        self.binary_path = Some(binary_path);
        Ok(())
    }

    fn run(&self, args: HashMap<String, Arg>, seed: u64) -> Result<Vec<u8>> {
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

        cmd_args.push("-seed".to_string());
        cmd_args.push(seed.to_string());

        let output = Command::new(binary)
            .args(&cmd_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            bail!("生成器运行失败：{}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(output.stdout)
    }
}
