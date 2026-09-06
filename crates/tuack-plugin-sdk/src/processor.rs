//! 处理器插件：trait 与注册宏。

use extism_pdk::Error;
use tuack_lib::ren::ProcessorOutput;
use tuack_ng_parser::ast::Document;

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
            $crate::__init_logger();
            let input: $crate::Json<$crate::Document> = match $crate::extism_pdk::input() {
                Ok(input) => input,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let processor = <$ty as $crate::Processor>::new();
            let output = match <$ty as $crate::Processor>::process(&processor, input.0) {
                Ok(output) => output,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            match $crate::extism_pdk::output($crate::Json(output)) {
                Ok(()) => 0,
                Err(e) => {
                    $crate::__report_error(&e);
                    -1
                }
            }
        }
    };
}
