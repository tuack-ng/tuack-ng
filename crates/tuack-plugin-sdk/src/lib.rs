//! tuack-ng 处理器/渲染器/导出器插件的 Rust SDK。
//!
//! 处理器插件实现 [`Processor`]，渲染器插件实现 [`Renderer`]，导出器插件实现
//! [`Dumper`]；分别用 [`processor`] / [`renderer`] / [`dumper`] 宏注册为
//! extism 导出函数，JSON 编解码、内存与错误处理均由 SDK 接管。
//!
//! 一个最小处理器插件（编译目标 `wasm32-unknown-unknown`）：
//! ```ignore
//! #![no_main]
//! use tuack_plugin_sdk::{processor, Document, Error, Processor, ProcessorOutput};
//!
//! struct MyProcessor;
//!
//! impl Processor for MyProcessor {
//!     fn new() -> Self {
//!         MyProcessor
//!     }
//!
//!     fn process(&self, doc: Document) -> Result<ProcessorOutput, Error> {
//!         Ok(ProcessorOutput {
//!             ast: doc,
//!             warnings: Vec::new(),
//!         })
//!     }
//! }
//!
//! processor!(MyProcessor);
//! ```

pub use extism_pdk;
pub use extism_pdk::{Error, Json, host_fn};
pub use log;
pub use tuack_lib::dump::{
    DumpCase, DumpChecker, DumpConfig, DumpDocument, DumpFile, DumpProblem, DumpSample,
    DumpSubtask, DumperOutput, ScorePolicy,
};
pub use tuack_lib::ren::{
    CommandResult, ProblemType, ProcessorOutput, RenderDocument, RendererOutput,
};
pub use tuack_lib::utils::output::OutputFile;
pub use tuack_ng_parser::ast::Document;

mod dumper;
mod host;
mod logger;
mod processor;
mod renderer;

pub use dumper::Dumper;
pub use host::{AssetReader, command, get_path};
pub use logger::__init_logger;
pub use processor::Processor;
pub use renderer::{Renderer, __write_outputs};

/// 将错误回传给宿主（供 [`processor`] / [`renderer`] / [`dumper`] 宏内部使用）。
#[doc(hidden)]
pub fn __report_error(e: &extism_pdk::Error) {
    let err = format!("{:?}", e);
    let mem = extism_pdk::Memory::from_bytes(&err).unwrap();
    unsafe {
        extism_pdk::extism::error_set(mem.offset());
    }
}
