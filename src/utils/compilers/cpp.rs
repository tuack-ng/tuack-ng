use std::process::Command as StdCommand;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::process::Command as TokioCommand;

use crate::prelude::*;
use crate::tuack_lib::utils::compiler::RunnerManifest;
use crate::utils::command::string_to_command;
use async_trait::async_trait;

pub struct CppRunner {
    tmp_dir: TempDir,
    source: PathBuf,
    file_io: bool,
    input_path: Option<PathBuf>,
    input_file_name: Option<String>,
    output_file_name: Option<String>,
    compile_args: String,
    program_name: String,
    interactive: bool,
    grader_path: Option<PathBuf>,
    header_path: Option<PathBuf>,
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
            file_io: false,
            input_path: None,
            input_file_name: None,
            output_file_name: None,
            compile_args: compile_args
                .get(&ext)
                .context("没有该语言编译选项")?
                .to_string(),
            program_name,
            interactive: false,
            grader_path: None,
            header_path: None,
        })
    }

    /// 获取编译命令（同步版本）
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

    /// 获取运行命令（同步版本）
    fn get_run_command(&self) -> Result<Option<StdCommand>> {
        let program_path = self.tmp_dir.path().join(&self.program_name);
        if program_path.exists() {
            Ok(Some(StdCommand::new(program_path)))
        } else {
            Ok(None)
        }
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
            fs::copy(&self.grader_path.as_ref().unwrap(), &grader_target_path)?;
            let header_target_path = self.tmp_dir.path().join(format!("{}.h", self.program_name));
            fs::copy(&self.header_path.as_ref().unwrap(), &header_target_path)?;
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
            tokio::fs::copy(&self.grader_path.as_ref().unwrap(), &grader_target_path).await?;
            let header_target_path = self.tmp_dir.path().join(format!("{}.h", self.program_name));
            tokio::fs::copy(&self.header_path.as_ref().unwrap(), &header_target_path).await?;
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

    fn get_run(&mut self) -> Result<StdCommand> {
        let mut cmd = self.get_run_command()?.context("无法获取运行命令")?;
        cmd.current_dir(&self.tmp_dir);

        Ok(if self.file_io {
            let input_path = self
                .tmp_dir
                .path()
                .join(self.input_file_name.as_ref().context("未指定输入文件名")?);
            fs::copy(
                self.input_path.as_ref().context("未指定输入文件")?,
                input_path,
            )?;

            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            cmd
        } else {
            let output_path = self
                .tmp_dir
                .path()
                .join(&self.program_name)
                .with_extension("stdout");
            cmd.stdin(Stdio::from(std::fs::File::open(
                self.input_path.as_ref().unwrap(),
            )?))
            .stdout(Stdio::from(std::fs::File::create(output_path)?))
            .stderr(Stdio::null());
            cmd
        })
    }

    async fn get_run_async(&mut self) -> Result<TokioCommand> {
        let mut cmd = TokioCommand::from(self.get_run_command()?.context("无法获取运行命令")?);
        cmd.current_dir(&self.tmp_dir);

        Ok(if self.file_io {
            let input_path = self
                .tmp_dir
                .path()
                .join(self.input_file_name.as_ref().context("未指定输入文件名")?);
            tokio::fs::copy(
                self.input_path.as_ref().context("未指定输入文件")?,
                input_path,
            )
            .await?;

            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            cmd
        } else {
            let output_path = self
                .tmp_dir
                .path()
                .join(&self.program_name)
                .with_extension("stdout");
            cmd.stdin(Stdio::from(std::fs::File::open(
                self.input_path.as_ref().context("未指定输入文件")?,
            )?))
            .stdout(Stdio::from(std::fs::File::create(output_path)?))
            .stderr(Stdio::null());
            cmd
        })
    }

    fn set_file_io(
        &mut self,
        input_file: &PathBuf,
        input_name: &String,
        output_name: &String,
    ) -> Result<()> {
        self.input_file_name = Some(input_name.to_owned());
        self.output_file_name = Some(output_name.to_owned());
        self.input_path = Some(input_file.to_owned());
        self.file_io = true;
        Ok(())
    }

    fn set_std_io(&mut self, input_file: &PathBuf) -> Result<()> {
        self.file_io = false;
        self.input_path = Some(input_file.to_owned());
        Ok(())
    }

    fn set_interactive(&mut self, grader_file: &PathBuf, header_file: &PathBuf) -> Result<()> {
        self.interactive = true;
        self.grader_path = Some(grader_file.to_owned());
        self.header_path = Some(header_file.to_owned());
        Ok(())
    }

    fn get_output_path(&self) -> Result<PathBuf> {
        Ok(if self.file_io {
            self.tmp_dir
                .path()
                .join(self.output_file_name.as_ref().context("未指定输出文件名")?)
        } else {
            self.tmp_dir
                .path()
                .join(&self.program_name)
                .with_extension("stdout")
        })
    }

    fn cleanup(&mut self) -> Result<()> {
        Ok(())
    }
}
