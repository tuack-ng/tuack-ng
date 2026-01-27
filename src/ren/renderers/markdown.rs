use crate::config::TemplateManifest;
use crate::prelude::*;
use crate::ren::Compiler;
use crate::ren::RenderQueue;
use crate::ren::copy_dir_recursive;
use crate::ren::renderers::base::Checker;
use markdown_ppp::printer::config::Config;
use markdown_ppp::printer::render_markdown;

pub struct MarkdownChecker {}

impl Checker for MarkdownChecker {
    fn new(_: PathBuf) -> Self {
        MarkdownChecker {}
    }

    fn check_compiler(&self) -> Result<()> {
        // Markdown 不需要特殊的编译器检查
        Ok(())
    }
}
pub struct MarkdownCompiler {
    pub tmp_dir: PathBuf,
    pub renderqueue: Vec<RenderQueue>,
}

impl Compiler for MarkdownCompiler {
    fn new(
        _: ContestConfig,
        _: ContestDayConfig,
        tmp_dir: PathBuf,
        renderqueue: Vec<RenderQueue>,
        _manifest: TemplateManifest,
    ) -> Self {
        MarkdownCompiler {
            tmp_dir,
            renderqueue,
        }
    }

    fn compile(&self) -> Result<PathBuf> {
        let output_dir = &self.tmp_dir.join("output");
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)?;
        }
        for item in &self.renderqueue {
            if let RenderQueue::Problem(ast, problem_config) = item {
                let output = render_markdown(ast, Config::default().with_width(10000000));
                let output_filename = format!("{}.md", problem_config.name);

                fs::write(output_dir.join(&output_filename), output)?;
                info!("生成 Markdown 文件: {}", output_filename);
            }
        }
        if self.tmp_dir.join("img").exists() {
            let target = output_dir.join("img");
            copy_dir_recursive(self.tmp_dir.join("img"), &target)?;
            info!("复制图片目录到: {}", target.display());
        }
        Ok(output_dir.clone())
    }
}
