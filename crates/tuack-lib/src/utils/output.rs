use crate::data::Reader;
use crate::prelude::*;

/// 渲染/导出产物：文件（路径 + 字节流）或空目录。
///
/// 产物（图片/PDF/数据）可能很大，文件以流承载，不整体进内存；
/// `Dir` 变体表达"该目录必须存在但可能没有文件"（如 Arbiter 的 final/players/result）。
///
/// 同一批产物中 `File` 的 `path` 应互不相同；宿主按路径落盘，重复路径的行为应视为未定义，不应依赖。
pub enum OutputFile {
    /// 文件：相对路径 + 字节流
    File {
        /// 相对路径，如 `img/a.png`、`main.typ`
        path: PathBuf,
        bytes: Box<dyn Reader>,
    },
    /// 空目录：确保存在
    Dir(PathBuf),
}

/// 插件回传的产物描述。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputSpec {
    /// 资产引用：以 `asset_id` 标识的资源，目标相对路径 `path`
    Asset { path: PathBuf, asset_id: u64 },
    /// 文件引用：内容位于临时工作区，目标相对路径 `path`
    File { path: PathBuf },
    /// 空目录：确保存在
    Dir(PathBuf),
}
