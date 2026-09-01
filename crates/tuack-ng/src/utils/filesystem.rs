use crate::prelude::*;
use std::fs;
use std::path::Path;
use tuack_lib::utils::output::OutputFile;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// 将产物写入输出目录（文件流式落盘，空目录直接创建）。
pub async fn write_outputs(base: &Path, files: Vec<OutputFile>) -> Result<()> {
    for file in files {
        match file {
            OutputFile::Dir(path) => {
                let target = base.join(&path);
                fs::create_dir_all(&target)
                    .with_context(|| format!("创建目录失败：{}", target.display()))?;
                info!("创建目录：{}", target.display());
            }
            OutputFile::File { path, bytes } => {
                let target = base.join(&path);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("创建目录失败：{}", parent.display()))?;
                }
                let mut bytes = bytes;
                let mut out = tokio::fs::File::create(&target)
                    .await
                    .with_context(|| format!("创建文件失败：{}", target.display()))?;
                tokio::io::copy(&mut bytes, &mut out)
                    .await
                    .with_context(|| format!("写入文件失败：{}", target.display()))?;
                info!("生成：{}", target.display());
            }
        }
    }
    Ok(())
}

pub fn copy_dir_recursive<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    if !dst.exists() {
        fs::create_dir_all(dst)?;
        #[cfg(unix)]
        add_write_permission(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let src_path = fs::canonicalize(&src_path)?;

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
            #[cfg(unix)]
            add_write_permission(&dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
            #[cfg(unix)]
            add_write_permission(&dst_path)?;
        }
    }

    Ok(())
}

pub fn create_or_clear_dir(path: &Path) -> Result<(), std::io::Error> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)
}

#[cfg(unix)]
fn add_write_permission(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();

    let mode = permissions.mode();
    permissions.set_mode(mode | 0o200);

    fs::set_permissions(path, permissions)?;
    Ok(())
}
