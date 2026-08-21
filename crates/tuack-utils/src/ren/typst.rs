use crate::prelude::*;
use crate::ren::manifest::TemplateManifest;
use crate::ren::renderers::{rewrite_images, unwrap_template};
use std::collections::HashSet;
use tuack_lib::ren::{ProblemType, RenderDocument, Renderer};
use tuack_lib::utils::output::OutputFile;
use tuack_ng_parser::printers::render_typst;

mod datajson;
use datajson::{DataJson, DateInfo, Problem, SupportLanguage};

/// Typst 渲染器
pub struct TypstRenderer {
    template_dir: PathBuf,
}

impl TypstRenderer {
    /// 解压模板到 `tmp_root` 并校验编译环境
    pub fn new(
        tmp_root: PathBuf,
        manifest: &TemplateManifest,
        assets_dirs: &[PathBuf],
    ) -> Result<Self> {
        unwrap_template(manifest, &tmp_root, assets_dirs)?;
        Self::check_typst_env(&tmp_root)?;
        Ok(Self {
            template_dir: tmp_root,
        })
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
            use_pretest: doc.config.use_pretest,
            noi_style: doc.config.noi_style,
            file_io: doc.config.file_io,
            support_languages,
            problems,
        }
    }

    /// 将各题图片流写入模板目录 img/ 下，供 typst 按相对路径引用（按目标路径去重）
    async fn write_images(
        &self,
        doc: &RenderDocument,
        images: &[(u64, PathBuf, PathBuf)],
    ) -> Result<()> {
        let img_dir = self.template_dir.join("img");
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
            let mut stream = doc.assets.load(*idx, url).await?;
            let mut file = tokio::fs::File::create(&dest).await?;
            tokio::io::copy(&mut stream, &mut file).await?;
            drop(file);
        }
        Ok(())
    }
}

#[async_trait]
impl Renderer for TypstRenderer {
    async fn render(&self, doc: &RenderDocument) -> Result<(PathBuf, Vec<OutputFile>)> {
        let day_key = doc.config.day_key.clone();

        let mut images = Vec::new();
        for problem in &doc.problems {
            let (ast, map) = rewrite_images(problem.ast.clone(), problem.idx)?;

            let typst_output = format!("#import \"utils.typ\": *\n{}", render_typst(&ast));
            tokio::fs::write(
                self.template_dir
                    .join(format!("problem-{}.typ", problem.idx)),
                typst_output,
            )
            .await?;

            for (url, target) in &map {
                images.push((problem.idx, url.clone(), target.clone()));
            }
        }

        if let Some(precaution) = &doc.precaution {
            let typst_output = format!("#import \"utils.typ\": *\n{}", render_typst(precaution));
            tokio::fs::write(self.template_dir.join("precaution.typ"), typst_output).await?;
        }

        let data_json = self.generate_conf(doc);
        let data_json_str = serde_json::to_string_pretty(&data_json)?;
        tokio::fs::write(self.template_dir.join("data.json"), data_json_str).await?;

        self.write_images(doc, &images).await?;

        fs::create_dir(self.template_dir.join("output"))?;

        let template_dir = self.template_dir.clone();
        let output_filename = format!("output/{}.pdf", day_key);
        let filename = output_filename.clone();
        let typst_output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("typst")
                .arg("compile")
                .arg("--font-path=fonts")
                .arg("main.typ")
                .arg(filename)
                .current_dir(&template_dir)
                .output()
        })
        .await?
        .context("typst 命令执行失败")?;

        if !typst_output.status.success() {
            let stderr = String::from_utf8_lossy(&typst_output.stderr).to_string();
            bail!(anyhow!(stderr).context("Typst 编译失败"));
        }

        let pdf_path = self.template_dir.join(output_filename);
        let bytes = tokio::fs::File::open(&pdf_path).await?;
        Ok((
            PathBuf::from(format!("{}.pdf", day_key)),
            vec![OutputFile::File {
                path: PathBuf::from(format!("{}.pdf", day_key)),
                bytes: Box::new(bytes),
            }],
        ))
    }
}
