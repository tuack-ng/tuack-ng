use std::io;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::prelude::*;
use crate::utils::testlib::Arg;

/// 统一的异步可读流抽象：`AsyncRead + Unpin + Send` 的便捷 trait
pub trait AsyncReader: AsyncRead + Unpin + Send {}

impl<T: AsyncRead + Unpin + Send> AsyncReader for T {}

/// 可读数据源
#[async_trait]
pub trait Data: Send {
    /// 数据点输入
    async fn input(&self) -> io::Result<Box<dyn AsyncReader>>;

    /// 数据点输出
    async fn answer(&self) -> io::Result<Box<dyn AsyncReader>>;
}

/// 可写数据点
#[async_trait]
pub trait DmkData: Data {
    /// 生成参数
    fn args(&self) -> &IndexMap<String, Arg>;

    /// 写入输入文件
    async fn write_input(&self, input: Box<dyn AsyncReader>) -> Result<()>;

    /// 写入输出文件
    async fn write_output(&self, output: Box<dyn AsyncReader>) -> Result<()>;
}
