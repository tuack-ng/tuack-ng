use sha2::{Digest, Sha256};
use std::{fs, path::Path};

// 为图片分配唯一ID并复制的函数
pub fn process_images_with_unique_ids(
    src_dir: &Path,
    dst_dir: &Path,
    _problem_idx: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if !dst_dir.exists() {
        fs::create_dir_all(dst_dir)?;
    }

    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let src_path = entry.path();

        if src_path.is_file() {
            // 计算文件的SHA256哈希值
            let mut file = std::fs::File::open(&src_path)?;
            let mut hasher = Sha256::new();
            std::io::copy(&mut file, &mut hasher)?;
            let hash = hasher.finalize();
            let hash_hex = format!("{:x}", hash);

            // 获取文件扩展名
            let extension = src_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("");

            // 生成唯一ID: sha256.extension
            let unique_filename = if extension.is_empty() {
                hash_hex
            } else {
                format!("{}.{}", hash_hex, extension)
            };
            let dst_path = dst_dir.join(unique_filename);

            // 复制文件
            fs::copy(&src_path, &dst_path)?;
            log::info!("复制图片: {} -> {}", src_path.display(), dst_path.display());
        }
    }

    Ok(())
}

// 递归复制目录的辅助函数
pub fn copy_dir_recursive<P: AsRef<Path>, Q: AsRef<Path>>(
    src: P,
    dst: Q,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
