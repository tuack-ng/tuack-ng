//! tuack-ng 处理器插件的 Rust SDK。
//!
//! 插件开发者实现 [`Processor`] trait，再用 [`processor`] 宏注册为
//! extism 导出函数；JSON 编解码、内存与错误处理均由 SDK 接管。
//!
//! 一个最小插件（编译目标 `wasm32-unknown-unknown`）：
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
pub use extism_pdk::{Error, Json};
pub use tuack_lib::ren::ProcessorOutput;
pub use tuack_ng_parser::ast::Document;

/// 插件处理器：实现它即可接入 extism，无需接触底层 ABI。
pub trait Processor: Send + Sync {
    /// 构造处理器实例。
    fn new() -> Self
    where
        Self: Sized;

    fn process(&self, doc: Document) -> Result<ProcessorOutput, Error>;
}

/// 注册处理器为 extism 导出函数（默认导出名 `process`）。
///
/// 宿主侧按 manifest 里 `function` 字段调用，缺省即 `process`。
#[macro_export]
macro_rules! processor {
    ($ty:ty) => {
        $crate::processor!($ty, process);
    };
    ($ty:ty, $name:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name() -> i32 {
            let input: $crate::Json<$crate::Document> = match $crate::extism_pdk::input() {
                Ok(input) => input,
                Err(e) => {
                    $crate::__processor_error(&e);
                    return -1;
                }
            };
            let processor = <$ty as $crate::Processor>::new();
            let output = match <$ty as $crate::Processor>::process(&processor, input.0) {
                Ok(output) => output,
                Err(e) => {
                    $crate::__processor_error(&e);
                    return -1;
                }
            };
            match $crate::extism_pdk::output($crate::Json(output)) {
                Ok(()) => 0,
                Err(e) => {
                    $crate::__processor_error(&e);
                    -1
                }
            }
        }
    };
}

/// 将错误回传给宿主（供 [`processor`] 宏内部使用）。
#[doc(hidden)]
pub fn __processor_error(e: &extism_pdk::Error) {
    let err = format!("{:?}", e);
    let mem = extism_pdk::Memory::from_bytes(&err).unwrap();
    unsafe {
        extism_pdk::extism::error_set(mem.offset());
    }
}
