//! 插件系统：插件包清单、管理器与 extism 宿主。

use path_clean::PathClean;

use crate::prelude::*;

pub(crate) mod extism;
pub(crate) mod factory;
pub mod manager;
pub mod manifest;

/// 校验 `rel` 为相对路径且规范化后不越出 `base`，返回相对 `base` 的规范化路径。
///
/// 用于约束插件清单路径、插件返回的产物路径等，拒绝绝对路径与 `..` 越界。
pub(crate) fn normalize_within(base: &Path, rel: &Path) -> Result<PathBuf> {
    if rel.is_absolute() {
        bail!("路径必须为相对路径：{}", rel.display());
    }
    let base = base.clean();
    let dest = base.join(rel).clean();
    dest.strip_prefix(&base)
        .map(Path::to_path_buf)
        .map_err(|_| anyhow!("路径越出基准目录：{}", rel.display()))
}
