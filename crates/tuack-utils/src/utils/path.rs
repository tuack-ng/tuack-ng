//! 路径安全校验：约束在基准目录内，拒绝绝对路径、`..` 越界与符号链接逃逸。

use path_clean::PathClean;

use crate::prelude::*;

/// 校验 `rel` 为相对路径且规范化后不越出 `base`，返回相对 `base` 的规范化路径。
///
/// 纯词法校验，用于约束清单路径、产物路径等。
pub fn normalize_within(base: &Path, rel: &Path) -> Result<PathBuf> {
    if rel.is_absolute() {
        bail!("路径必须为相对路径：{}", rel.display());
    }
    let base = base.clean();
    let dest = base.join(rel).clean();
    dest.strip_prefix(&base)
        .map(Path::to_path_buf)
        .map_err(|_| anyhow!("路径越出基准目录：{}", rel.display()))
}

/// 校验 `path`（已词法约束在 `base` 内）不含符号链接，防止经软链越界读写。
///
/// 逐段 `symlink_metadata`（不跟随），对悬空软链同样有效；普通文件/目录或尚不存在的
/// 分量放行。
pub fn assert_within(base: &Path, path: &Path) -> Result<()> {
    let base = base.clean();
    let rel = path
        .strip_prefix(&base)
        .map_err(|_| anyhow!("路径越出基准目录：{}", path.display()))?;
    let mut cur = base;
    for comp in rel.components() {
        cur.push(comp);
        if let Ok(meta) = fs::symlink_metadata(&cur)
            && meta.file_type().is_symlink()
        {
            bail!("路径含符号链接：{}", cur.display());
        }
    }
    Ok(())
}

/// `normalize_within` + `assert_within`，返回可安全访问的绝对宿主路径。
pub fn resolve_within(base: &Path, rel: &Path) -> Result<PathBuf> {
    let base = base.clean();
    let rel = normalize_within(&base, rel)?;
    let path = base.join(&rel);
    assert_within(&base, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_within_rejects_absolute_and_escape() {
        let base = Path::new("/tmp/base");
        assert!(normalize_within(base, Path::new("a/b")).is_ok());
        assert!(normalize_within(base, Path::new("/abs")).is_err());
        assert!(normalize_within(base, Path::new("../x")).is_err());
        assert!(normalize_within(base, Path::new("a/../../x")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn assert_within_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let outside = tempfile::tempdir().unwrap();

        // 普通路径与尚不存在的路径放行
        assert!(assert_within(base, &base.join("a/b")).is_ok());

        // 指向外部已存在目标的软链
        std::fs::write(outside.path().join("secret"), b"x").unwrap();
        symlink(outside.path().join("secret"), base.join("link")).unwrap();
        assert!(assert_within(base, &base.join("link")).is_err());

        // 悬空软链（旧 canonicalize 实现会漏掉）
        symlink(outside.path().join("missing"), base.join("dangling")).unwrap();
        assert!(assert_within(base, &base.join("dangling")).is_err());

        // 中间目录为软链
        symlink(outside.path(), base.join("dir")).unwrap();
        assert!(assert_within(base, &base.join("dir/x")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_within_returns_path_and_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        assert_eq!(
            resolve_within(base, Path::new("out/x")).unwrap(),
            base.join("out/x")
        );

        symlink("/etc/passwd", base.join("evil")).unwrap();
        assert!(resolve_within(base, Path::new("evil")).is_err());
    }
}
