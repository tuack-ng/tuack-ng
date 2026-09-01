use crate::prelude::*;
use crate::ren::renderers::rewrite_images;
use std::collections::HashSet;
use tuack_lib::ren::{RenderDocument, Renderer};
use tuack_lib::utils::output::OutputFile;
use tuack_ng_parser::printers::render_markdown;

/// Markdown 渲染器
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Renderer for MarkdownRenderer {
    async fn render(&self, doc: &RenderDocument) -> Result<(PathBuf, Vec<OutputFile>)> {
        let mut files = Vec::new();
        for problem in &doc.problems {
            let (ast, images) = rewrite_images(problem.ast.clone(), problem.idx)?;

            let output = render_markdown(&ast);
            files.push(OutputFile::File {
                path: PathBuf::from(format!("{}/{}.md", doc.config.day_key, problem.meta.name)),
                bytes: Box::new(std::io::Cursor::new(output.into_bytes())),
            });

            let mut seen = HashSet::new();
            for (url, target) in &images {
                if !seen.insert(target.clone()) {
                    continue;
                }
                let stream = doc.assets.load(problem.idx, url).await?;
                files.push(OutputFile::File {
                    path: PathBuf::from(format!("{}/{}", doc.config.day_key, target.display())),
                    bytes: stream,
                });
            }
        }
        Ok((PathBuf::from(&doc.config.day_key), files))
    }
}
