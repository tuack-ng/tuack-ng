//! 通用工具：路径安全校验、临时目录保活等。

pub mod keepalive;
pub mod path;

pub use keepalive::KeepAliveReader;
pub use path::{assert_within, normalize_within, resolve_within};
