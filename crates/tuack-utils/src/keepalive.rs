//! 产物返回时的临时目录保活包装。

use std::io::Read;
use std::sync::Arc;

use tempfile::TempDir;

/// 包装一个文件流并持有临时目录，保证底层文件在流被消费前不被回收。
///
/// 产出方若把临时目录中的文件作为产物返回（返回的流指向该文件），应使用本包装，
/// 使临时目录的存活期跟随产物本身，而不依赖调用顺序或“已删除目录中的 fd 仍可读”
/// 这类操作系统行为。
pub struct KeepAliveReader {
    inner: std::fs::File,
    _keepalive: Arc<TempDir>,
}

impl KeepAliveReader {
    pub fn new(inner: std::fs::File, keepalive: Arc<TempDir>) -> Self {
        Self {
            inner,
            _keepalive: keepalive,
        }
    }
}

impl Read for KeepAliveReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}
