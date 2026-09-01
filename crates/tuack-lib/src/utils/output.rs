use std::path::PathBuf;

use crate::data::AsyncReader;

/// 渲染/导出产物：文件（路径 + 字节流）或空目录。
///
/// 产物（图片/PDF/数据）可能很大，文件以流承载，不整体进内存；
/// `Dir` 变体表达"该目录必须存在但可能没有文件"（如 Arbiter 的 final/players/result）。
pub enum OutputFile {
    /// 文件：相对路径 + 字节流
    File {
        /// 相对路径，如 `img/a.png`、`main.typ`
        path: PathBuf,
        bytes: Box<dyn AsyncReader>,
    },
    /// 空目录：确保存在
    Dir(PathBuf),
}
