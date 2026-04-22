use std::process::Command as StdCommand;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::process::Command as TokioCommand;

use crate::utils::command::string_to_command;
use crate::{prelude::*, tuack_lib::config::lang::Language};
use strfmt::strfmt;

pub struct GeneralRunner {
    tmp_dir: TempDir,
    source: PathBuf,
    file_io: bool,
    input_path: Option<PathBuf>,
    input_file_name: Option<String>,
    output_file_name: Option<String>,
    compile_args: String,
    language: Language,
    program_name: String,
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
            file_io: false,
            input_path: None,
            input_file_name: None,
            output_file_name: None,
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
        })
    }

    /// 获取编译命令（同步版本）
    fn get_compile_command(&self) -> Result<Option<StdCommand>> {
        if let Some(ref compile) = self.language.compiler {
            let compile_cmd = strfmt(
                &compile.run,
                &HashMap::from([
                    ("executable".to_string(), compile.executable.clone()),
                    (
                        "output_path".to_string(),
                        self.tmp_dir.path().to_string_lossy().to_string(),
                    ),
                    ("program_name".to_string(), self.program_name.clone()),
                    ("args".to_string(), self.compile_args.clone()),
                    (
                        "input_path".to_string(),
                        self.source.to_string_lossy().to_string(),
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

    /// 获取运行命令（同步版本）
    fn get_run_command(&self) -> Result<Option<StdCommand>> {
        if let Some(ref runner) = self.language.runner {
            let run_cmd = strfmt(
                &runner.run,
                &HashMap::from([
                    ("executable".to_string(), runner.executable.clone()),
                    (
                        "input_path".to_string(),
                        self.tmp_dir.path().to_string_lossy().to_string(),
                    ),
                    ("program_name".to_string(), self.program_name.clone()),
                    (
                        "exe_suffix".to_string(),
                        std::env::consts::EXE_SUFFIX.to_string(),
                    ),
                ]),
            )?;
            Ok(Some(string_to_command(run_cmd.as_str())?))
        } else {
            let program_path = self.tmp_dir.path().join(&self.program_name);
            if program_path.exists() {
                Ok(Some(StdCommand::new(program_path)))
            } else {
                Ok(None)
            }
        }
    }
}

impl Runner for GeneralRunner {
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
