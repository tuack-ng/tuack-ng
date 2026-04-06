use crate::prelude::*;
use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
