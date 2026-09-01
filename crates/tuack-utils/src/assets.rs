use crate::prelude::*;
use tuack_lib::data::AsyncReader;
use tuack_lib::utils::asset::AssetProvider;

/// 前端资源提供方：登记 `题目编号 -> 题目路径` 映射，
/// `load` 按 idx + 相对路径 join 打开文件返回流
///
/// 供 ren（图片）与 dump（数据/样例）共用，惰性打开不预载。
pub struct FsAssetProvider {
    /// 题目编号 -> 题目路径
    dirs: HashMap<u64, PathBuf>,
}

impl FsAssetProvider {
    pub fn new() -> Self {
        Self {
            dirs: HashMap::new(),
        }
    }

    /// 登记某题的资源基准目录（题目路径）
    pub fn register(&mut self, idx: u64, problem_path: PathBuf) {
        self.dirs.insert(idx, problem_path);
    }
}

impl Default for FsAssetProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AssetProvider for FsAssetProvider {
    async fn load(&self, idx: u64, path: &Path) -> Result<Box<dyn AsyncReader>> {
        use std::path::Component;

        let base = self
            .dirs
            .get(&idx)
            .context(format!("题目 {} 未登记资源目录", idx))?;
        let traversal = path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });
        if traversal {
            bail!("资源路径不合法：{}，不允许目录穿越", path.display());
        }
        let src = base.join(path);
        let file = tokio::fs::File::open(&src)
            .await
            .with_context(|| format!("打开资源失败：{}", src.display()))?;
        Ok(Box::new(file))
    }
}
