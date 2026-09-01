use crate::ren::manifest::TemplateManifest;
use tuack_ng_parser::ast::Document;
use tuack_ng_parser::ast::inline::Image;
use tuack_ng_parser::transform::Transform;
use tuack_ng_parser::visitor::{VisitWith, Visitor};

use crate::prelude::*;

/// 收集文档中引用的所有图片 URL（去重）
#[derive(Default)]
pub struct ImageCollector {
    pub urls: Vec<String>,
}

impl ImageCollector {
    pub fn collect(doc: &Document) -> Vec<String> {
        let mut collector = ImageCollector::default();
        doc.visit_with(&mut collector);
        collector.urls
    }
}

impl Visitor for ImageCollector {
    fn visit_image(&mut self, image: &Image) {
        if !self.urls.contains(&image.destination) {
            self.urls.push(image.destination.clone());
        }
        self.walk_image(image);
    }
}

/// 检查并重写文档中的图片 URL，返回 `(重写后的 AST, 原始 URL -> 目标 URL 映射)`。
/// 映射以 `PathBuf` 承载，`AssetProvider::load` 直接消费。
pub fn rewrite_images(ast: Document, idx: u64) -> Result<(Document, IndexMap<PathBuf, PathBuf>)> {
    use std::path::Component;

    let mut ast = ast;
    let mut map = IndexMap::new();
    let mut invalid: Vec<String> = Vec::new();
    ast.transform_image_urls(|url| {
        if let Some(rel) = url
            .strip_prefix("./img/")
            .or_else(|| url.strip_prefix("img/"))
        {
            let traversal = Path::new(rel).components().any(|c| {
                matches!(
                    c,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            });
            if traversal {
                invalid.push(format!("图片 URL 不合法：{}，不允许目录穿越", url));
                url.to_string()
            } else {
                let target = format!("img/{}/{}", idx, rel);
                map.entry(PathBuf::from(url))
                    .or_insert_with(|| PathBuf::from(&target));
                target
            }
        } else {
            invalid.push(format!("图片 URL 不合法：{}，只支持 img/ 下的图片", url));
            url.to_string()
        }
    });
    if !invalid.is_empty() {
        bail!("{}", invalid.join("\n"));
    }
    Ok((ast, map))
}

/// 按 manifest.filelist 从 assets store 解压模板到目标目录
pub fn unwrap_template(
    manifest: &TemplateManifest,
    output_dir: &Path,
    assets_dirs: &[PathBuf],
) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    for (relative_path, sha256) in &manifest.filelist {
        let source_file = find_file_in_store(assets_dirs, sha256)
            .with_context(|| format!("查找文件失败：{} (sha256: {})", relative_path, sha256))?;

        let target_file = output_dir.join(relative_path);
        if let Some(parent) = target_file.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(&source_file, &target_file).with_context(|| {
            format!(
                "复制文件失败：{} -> {}",
                source_file.display(),
                target_file.display()
            )
        })?;
    }

    Ok(())
}

/// 在所有 assets 目录中查找文件（按优先级）
fn find_file_in_store(assets_dirs: &[PathBuf], sha256: &str) -> Result<PathBuf> {
    for assets_dir in assets_dirs {
        let file_path = assets_dir.join("templates").join("store").join(sha256);
        if file_path.exists() && file_path.is_file() {
            return Ok(file_path);
        }
    }

    bail!("在所有 assets 目录中未找到文件：{}", sha256)
}
