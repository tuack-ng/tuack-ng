use crate::context::gctx;
use crate::ren::TemplateManifest;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
/// 解包模板到目标目录
pub fn unwrap_template(manifest: &TemplateManifest, output_dir: &Path) -> Result<()> {
    // 获取 assets_dirs
    let assets_dirs = &gctx().assets_dirs;

    // 创建输出目录
    fs::create_dir_all(output_dir)?;

    // 解包所有文件
    for (relative_path, sha256) in &manifest.filelist {
        // 为每个文件查找 store 目录
        let source_file = find_file_in_store(assets_dirs, sha256)
            .with_context(|| format!("查找文件失败: {} (sha256: {})", relative_path, sha256))?;

        let target_file = output_dir.join(relative_path);

        // 创建父目录
        if let Some(parent) = target_file.parent() {
            fs::create_dir_all(parent)?;
        }

        // 复制文件
        fs::copy(&source_file, &target_file).with_context(|| {
            format!(
                "复制文件失败: {} -> {}",
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

    bail!("在所有 assets 目录中未找到文件: {}", sha256)
}
