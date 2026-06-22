use std::process::Command as StdCommand;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::process::Command as TokioCommand;

use crate::prelude::*;
use crate::tuack_lib::utils::compiler::{IoMode, ResourceLimits, RunResult, RunnerManifest};
use crate::utils::command::string_to_command;
use crate::utils::process::ProcessSupervisor;
use async_trait::async_trait;

pub struct CppRunner {
    tmp_dir: TempDir,
    source: PathBuf,
    compile_args: String,
    program_name: String,
    interactive: bool,
    grader_path: Option<PathBuf>,
    header_path: Option<PathBuf>,
    limits: Option<ResourceLimits>,
    input: Option<Vec<u8>>,
    io_mode: IoMode,
}

impl CppRunner {
    pub fn new(
        source: impl Into<PathBuf>,
        compile_args: &HashMap<String, String>,
        program_name: impl Into<String>,
    ) -> Result<Self> {
        let source = source.into();
        let program_name = program_name.into();
        let tmp_dir = TempDir::with_prefix("tuack-ng-runner-")?;
        let ext = source
            .extension()
            .context("没有后缀名")?
            .to_string_lossy()
            .into_owned();
        Ok(CppRunner {
            tmp_dir,
            source,
            compile_args: compile_args
                .get(&ext)
                .context("没有该语言编译选项")?
                .to_string(),
            program_name,
            interactive: false,
            grader_path: None,
            header_path: None,
            limits: None,
            input: None,
            io_mode: IoMode::Stdio,
        })
    }

    fn get_compile_command(&self) -> Result<StdCommand> {
        let exe_path = self
            .tmp_dir
            .path()
            .join(format!(
                "{}{}",
                self.program_name,
                std::env::consts::EXE_SUFFIX
            ))
            .display()
            .to_string()
            .replace(" ", "\\ ");

        let source_path = self.source.display().to_string().replace(" ", "\\ ");

        let mut cmd_str = format!("g++ -o {} {} {}", exe_path, self.compile_args, source_path);

        if self.interactive {
            let grader_path = self
                .tmp_dir
                .path()
                .join("grader.cpp")
                .display()
                .to_string()
                .replace(" ", "\\ ");
            cmd_str = format!("{} {}", cmd_str, grader_path);
        }

        string_to_command(&cmd_str)
    }
}

#[async_trait]
impl Runner for CppRunner {
    fn manifest(&self) -> RunnerManifest {
        RunnerManifest { interactive: true }
    }

    fn prepare(&mut self) -> Result<()> {
        if !self.tmp_dir.path().exists() {
            fs::create_dir_all(&self.tmp_dir)?;
        }

        let mut cmd = self.get_compile_command()?;

        let source_target_path = self
            .tmp_dir
            .path()
            .join(&self.program_name)
            .with_extension(self.source.extension().unwrap());

        fs::copy(&self.source, &source_target_path)?;

        if self.interactive {
            let grader_target_path = self.tmp_dir.path().join("grader.cpp");
            fs::copy(self.grader_path.as_ref().unwrap(), &grader_target_path)?;
            let header_target_path = self.tmp_dir.path().join(format!("{}.h", self.program_name));
            fs::copy(self.header_path.as_ref().unwrap(), &header_target_path)?;
        }

        let output = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output()?;
        if !output.status.success() {
            bail!("编译错误: {}", String::from_utf8_lossy(&output.stderr));
        }

        fs::remove_file(&source_target_path)?;

        Ok(())
    }

    async fn prepare_async(&mut self) -> Result<()> {
        if !self.tmp_dir.path().exists() {
            tokio::fs::create_dir_all(&self.tmp_dir).await?;
        }

        let cmd = self.get_compile_command()?;

        let target_path = self
            .tmp_dir
            .path()
            .join(&self.program_name)
            .with_extension(self.source.extension().unwrap());

        tokio::fs::copy(&self.source, &target_path).await?;

        if self.interactive {
            let grader_target_path = self.tmp_dir.path().join("grader.cpp");
            tokio::fs::copy(self.grader_path.as_ref().unwrap(), &grader_target_path).await?;
            let header_target_path = self.tmp_dir.path().join(format!("{}.h", self.program_name));
            tokio::fs::copy(self.header_path.as_ref().unwrap(), &header_target_path).await?;
        }

        let mut tokio_cmd = TokioCommand::from(cmd);
        let output = tokio_cmd
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await?;
        if !output.status.success() {
            bail!("编译错误: {}", String::from_utf8_lossy(&output.stderr));
        }

        tokio::fs::remove_file(&target_path).await?;

        Ok(())
    }

    fn set_limits(&mut self, limits: ResourceLimits) {
        self.limits = Some(limits);
    }

    fn set_input(&mut self, input: Vec<u8>) {
        self.input = Some(input);
    }

    fn set_io_mode(&mut self, io_mode: IoMode) {
        self.io_mode = io_mode;
    }

    fn set_interactive(&mut self, grader_file: &PathBuf, header_file: &PathBuf) -> Result<()> {
        self.interactive = true;
        self.grader_path = Some(grader_file.to_owned());
        self.header_path = Some(header_file.to_owned());
        Ok(())
    }

    async fn execute(&mut self) -> Result<RunResult> {
        let limits = self.limits.take().unwrap_or(ResourceLimits::unlimited());

        let input_buf = self.input.take().unwrap_or_default();

        let program_path = self.tmp_dir.path().join(&self.program_name);
        if !program_path.exists() {
            bail!("可执行文件不存在: {}", program_path.display());
        }

        let mut cmd = StdCommand::new(&program_path);
        cmd.current_dir(&self.tmp_dir);

        // 根据 IO 模式设置 stdin/stdout
        match &self.io_mode {
            IoMode::Stdio => {
                let stdin_path = self.tmp_dir.path().join("pipe_stdin");
                let stdout_path = self.tmp_dir.path().join("pipe_stdout");
                std::fs::write(&stdin_path, &input_buf)?;
                let stdin_file = std::fs::File::open(&stdin_path)?;
                let stdout_file = std::fs::File::create(&stdout_path)?;
                cmd.stdin(Stdio::from(stdin_file));
                cmd.stdout(Stdio::from(stdout_file));
            }
            IoMode::File { input_name, .. } => {
                let input_path = self.tmp_dir.path().join(input_name);
                std::fs::write(&input_path, &input_buf)?;
                cmd.stdin(Stdio::null());
                cmd.stdout(Stdio::null());
            }
        }
        cmd.stderr(Stdio::piped());

        let mut tokio_cmd = TokioCommand::from(cmd);
        let mut child = tokio_cmd.spawn()?;

        let (status, time, memory) = ProcessSupervisor::new(limits).supervise(&mut child).await?;

        // 读取 stderr
        let mut stderr = Vec::new();
        if let Some(mut err) = child.stderr.take() {
            let _ = err.read_to_end(&mut stderr).await;
        }

        // 读取 output
        let output = match &self.io_mode {
            IoMode::Stdio => tokio::fs::read(self.tmp_dir.path().join("pipe_stdout"))
                .await
                .unwrap_or_default(),
            IoMode::File { output_name, .. } => {
                tokio::fs::read(self.tmp_dir.path().join(output_name))
                    .await
                    .unwrap_or_default()
            }
        };

        Ok(RunResult {
            status,
            time,
            memory,
            output,
            stderr,
        })
    }

    fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }
}
