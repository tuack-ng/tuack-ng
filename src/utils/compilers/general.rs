use std::process::Command as StdCommand;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::process::Command as TokioCommand;

use crate::prelude::*;
use crate::tuack_lib::config::lang::Language;
use crate::tuack_lib::utils::compiler::{IoMode, ResourceLimits, RunResult, RunnerManifest};
use crate::utils::command::string_to_command;
use crate::utils::process::ProcessSupervisor;
use async_trait::async_trait;
use strfmt::strfmt;

pub struct GeneralRunner {
    tmp_dir: TempDir,
    source: PathBuf,
    compile_args: String,
    language: Language,
    program_name: String,
    limits: Option<ResourceLimits>,
    input: Option<Vec<u8>>,
    io_mode: IoMode,
}

impl GeneralRunner {
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
        Ok(GeneralRunner {
            tmp_dir,
            source,
            compile_args: compile_args
                .get(&ext)
                .context("没有该语言编译选项")?
                .to_string(),
            language: gctx()
                .languages
                .get(&ext)
                .context("未知格式文件")?
                .to_owned(),
            program_name,
            limits: None,
            input: None,
            io_mode: IoMode::Stdio,
        })
    }

    fn get_compile_command(&self) -> Result<Option<StdCommand>> {
        if let Some(ref compile) = self.language.compiler {
            let compile_cmd = strfmt(
                &compile.run,
                &HashMap::from([
                    ("executable".to_string(), compile.executable.clone()),
                    (
                        "output_path".to_string(),
                        self.tmp_dir
                            .path()
                            .to_string_lossy()
                            .to_string()
                            .replace(" ", "\\ "),
                    ),
                    (
                        "program_name".to_string(),
                        self.program_name.clone().replace(" ", "\\ "),
                    ),
                    ("args".to_string(), self.compile_args.clone()),
                    (
                        "input_path".to_string(),
                        self.source
                            .to_string_lossy()
                            .to_string()
                            .replace(" ", "\\ "),
                    ),
                    (
                        "exe_suffix".to_string(),
                        std::env::consts::EXE_SUFFIX.to_string(),
                    ),
                ]),
            )?;
            Ok(Some(string_to_command(compile_cmd.as_str())?))
        } else {
            Ok(None)
        }
    }

    fn get_run_base_command(&self) -> Result<StdCommand> {
        if let Some(ref runner) = self.language.runner {
            let run_cmd = strfmt(
                &runner.run,
                &HashMap::from([
                    ("executable".to_string(), runner.executable.clone()),
                    (
                        "input_path".to_string(),
                        self.tmp_dir
                            .path()
                            .to_string_lossy()
                            .to_string()
                            .replace(" ", "\\ "),
                    ),
                    ("program_name".to_string(), self.program_name.clone()),
                    (
                        "exe_suffix".to_string(),
                        std::env::consts::EXE_SUFFIX.to_string(),
                    ),
                ]),
            )?;
            Ok(string_to_command(run_cmd.as_str())?)
        } else {
            let program_path = self.tmp_dir.path().join(&self.program_name);
            if !program_path.exists() {
                bail!("可执行文件不存在: {}", program_path.display());
            }
            Ok(StdCommand::new(program_path))
        }
    }
}

#[async_trait]
impl Runner for GeneralRunner {
    fn manifest(&self) -> RunnerManifest {
        RunnerManifest { interactive: false }
    }

    fn prepare(&mut self) -> Result<()> {
        if !self.tmp_dir.path().exists() {
            fs::create_dir_all(&self.tmp_dir)?;
        }

        if let Some(mut cmd) = self.get_compile_command()? {
            let target_path = self
                .tmp_dir
                .path()
                .join(&self.program_name)
                .with_extension(self.source.extension().unwrap());

            fs::copy(&self.source, &target_path)?;

            debug!("{cmd:#?}");

            let output = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).output()?;
            debug!("??");
            if !output.status.success() {
                bail!("编译错误: {}", String::from_utf8_lossy(&output.stderr));
            }

            fs::remove_file(&target_path)?;
        } else {
            let target_path = self
                .tmp_dir
                .path()
                .join(self.program_name.clone())
                .with_extension(self.source.extension().unwrap());
            fs::copy(&self.source, target_path)?;
        }

        Ok(())
    }

    async fn prepare_async(&mut self) -> Result<()> {
        if !self.tmp_dir.path().exists() {
            tokio::fs::create_dir_all(&self.tmp_dir).await?;
        }

        if let Some(cmd) = self.get_compile_command()? {
            let target_path = self
                .tmp_dir
                .path()
                .join(&self.program_name)
                .with_extension(self.source.extension().unwrap());

            tokio::fs::copy(&self.source, &target_path).await?;

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
        } else {
            let target_path = self
                .tmp_dir
                .path()
                .join(self.program_name.clone())
                .with_extension(self.source.extension().unwrap());
            tokio::fs::copy(&self.source, target_path).await?;
        }

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

    fn set_interactive(&mut self, _grader_file: &PathBuf, _header_file: &PathBuf) -> Result<()> {
        unreachable!("通用运行器不支持交互");
    }

    async fn execute(&mut self) -> Result<RunResult> {
        let limits = self.limits.take().unwrap_or(ResourceLimits::unlimited());

        let input_buf = self.input.take().unwrap_or_default();

        let mut cmd = self.get_run_base_command()?;
        cmd.current_dir(&self.tmp_dir);

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

        let mut stderr = Vec::new();
        if let Some(mut err) = child.stderr.take() {
            let _ = err.read_to_end(&mut stderr).await;
        }

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
