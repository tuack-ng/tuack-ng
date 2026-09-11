use crate::keepalive::KeepAliveReader;
use crate::prelude::*;
use std::collections::HashSet;
use tempfile::TempDir;
use tuack_lib::ren::{ProblemType, RenderDocument, Renderer};
use tuack_lib::utils::asset::AssetProvider;
use tuack_lib::utils::output::OutputFile;
use tuack_ng_parser::printers::render_typst;

mod datajson;
use datajson::{DataJson, DateInfo, Problem, SupportLanguage};

/// Typst 渲染器
pub struct TypstRenderer {
    tmp: Arc<TempDir>,
    /// 模板/工作目录（WASI 工作区 `/tmp`），由外部预先落好模板文件
    work_dir: PathBuf,
}

impl TypstRenderer {
    /// 校验编译环境；模板文件须已落到 `tmp/tmp`。
    pub fn new(tmp: Arc<TempDir>) -> Result<Self> {
        let work_dir = tmp.path().join("tmp");
        Self::check_typst_env(&work_dir)?;
        Ok(Self { tmp, work_dir })
    }

    /// 校验 typst 命令可用且模板文件齐全
    fn check_typst_env(template_dir: &Path) -> Result<()> {
        debug!("检查 Typst 编译环境");
        let typst_check = std::process::Command::new("typst")
            .arg("--version")
            .output();

        match typst_check {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    debug!("Typst 版本：{}", version.trim());
                } else {
                    bail!("Typst 命令执行失败，请检查是否已安装");
                }
            }
            Err(e) => {
                bail!(anyhow!(e).context("未找到 typst 命令，请确保已安装并添加到 PATH"));
            }
        }

        let template_required_files = ["main.typ", "utils.typ"];
        for file in template_required_files {
            if !template_dir.join(file).exists() {
                bail!("模板缺少必要文件：{}", file);
            }
            info!("文件存在：{}", file);
        }
        Ok(())
    }

    fn generate_conf(&self, doc: &RenderDocument) -> DataJson {
        let problems = doc
            .problems
            .iter()
            .map(|p| {
                let meta = &p.meta;
                let problem_type = match meta.problem_type {
                    ProblemType::Program => "传统型",
                    ProblemType::Output => "提交答案型",
                    ProblemType::Interactive => "交互型",
                };
                Problem {
                    name: meta.name.clone(),
                    title: meta.title.clone(),
                    dir: meta.name.clone(),
                    exec: meta.name.clone(),
                    input: format!("{}.in", meta.name),
                    output: format!("{}.out", meta.name),
                    problem_type: problem_type.to_string(),
                    time_limit: format!("{:.1} 秒", meta.time_limit.as_secs_f64()),
                    memory_limit: format!("{:.0}", meta.memory_limit),
                    testcase: meta.testcase.to_string(),
                    point_equal: if meta.point_equal { "是" } else { "否" }.to_string(),
                    submit_filename: meta.submit_filename.clone(),
                }
            })
            .collect();
        let support_languages = doc
            .config
            .support_languages
            .iter()
            .map(|l| SupportLanguage {
                name: l.name.clone(),
                compile_options: l.compile_options.clone(),
            })
            .collect();
        DataJson {
            title: doc.config.title.clone(),
            subtitle: doc.config.short_title.clone(),
            dayname: doc.config.dayname.clone(),
            date: doc.config.date.map(|d| DateInfo {
                start: d.start,
                end: d.end,
            }),
            use_pretest: doc.config.params.use_pretest,
            noi_style: doc.config.params.noi_style,
            file_io: doc.config.params.file_io,
            support_languages,
            problems,
        }
    }

    /// 将各题图片流写入模板目录 img/ 下，供 typst 按相对路径引用（按目标路径去重）
    fn write_images(
        &self,
        assets: &dyn AssetProvider,
        images: &[(u64, PathBuf, PathBuf)],
    ) -> Result<()> {
        let img_dir = self.work_dir.join("img");
        let mut seen = HashSet::new();
        for (idx, url, target) in images {
            if !seen.insert(target.clone()) {
                continue;
            }
            let rel = target
                .strip_prefix("img/")
                .context(format!("图片路径不合法：{}", target.display()))?;
            let dest = img_dir.join(rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut stream = assets.load(*idx, url)?;
            let mut file = std::fs::File::create(&dest)?;
            std::io::copy(&mut stream, &mut file)?;
        }
        Ok(())
    }
}

impl Renderer for TypstRenderer {
    fn render(
        &self,
        doc: &RenderDocument,
        assets: Box<dyn AssetProvider>,
    ) -> Result<(PathBuf, Vec<OutputFile>)> {
        let day_key = doc.config.day_key.clone();

        let mut images = Vec::new();
        for problem in &doc.problems {
            let typst_output = format!("#import \"utils.typ\": *\n{}", render_typst(&problem.ast));
            fs::write(
                self.work_dir.join(format!("problem-{}.typ", problem.idx)),
                typst_output,
            )?;

            for (url, target) in &problem.images {
                images.push((problem.idx, url.clone(), target.clone()));
            }
        }

        if let Some(precaution) = &doc.precaution {
            let typst_output = format!("#import \"utils.typ\": *\n{}", render_typst(precaution));
            fs::write(self.work_dir.join("precaution.typ"), typst_output)?;
        }

        let data_json = self.generate_conf(doc);
        let data_json_str = serde_json::to_string_pretty(&data_json)?;
        fs::write(self.work_dir.join("data.json"), data_json_str)?;

        self.write_images(&*assets, &images)?;

        fs::create_dir(self.work_dir.join("output"))?;

        let output_filename = format!("output/{}.pdf", day_key);
        let typst_output = std::process::Command::new("typst")
            .arg("compile")
            .arg("--font-path=fonts")
            .arg("main.typ")
            .arg(&output_filename)
            .current_dir(&self.work_dir)
            .output()
            .context("typst 命令执行失败")?;

        if !typst_output.status.success() {
            let stderr = String::from_utf8_lossy(&typst_output.stderr).to_string();
            bail!(anyhow!(stderr).context("Typst 编译失败"));
        }

        let pdf_path = self.work_dir.join(output_filename);
        let bytes = KeepAliveReader::new(std::fs::File::open(&pdf_path)?, self.tmp.clone());
        Ok((
            PathBuf::from(format!("{}.pdf", day_key)),
            vec![OutputFile::File {
                path: PathBuf::from(format!("{}.pdf", day_key)),
                bytes: Box::new(bytes),
            }],
        ))
    }
}
