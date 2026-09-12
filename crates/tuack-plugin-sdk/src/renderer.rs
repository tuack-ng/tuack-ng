//! 渲染器插件：trait 与注册宏。

use std::path::PathBuf;

use extism_pdk::Error;
use tuack_lib::ren::RenderDocument;
use tuack_lib::utils::output::OutputFile;

/// 插件渲染器：实现它即可接入 extism。
///
/// `render` 接收渲染文档，返回主产物相对路径与产物文件列表。
pub trait Renderer: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    fn render(&self, doc: RenderDocument) -> Result<(PathBuf, Vec<OutputFile>), Error>;
}

/// 注册渲染器为 extism 导出函数（默认导出名 `render`）。
#[macro_export]
macro_rules! renderer {
    ($ty:ty) => {
        $crate::renderer!($ty, render);
    };
    ($ty:ty, $name:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name() -> i32 {
            $crate::__init_logger();
            let input: $crate::Json<$crate::RenderDocument> = match $crate::extism_pdk::input() {
                Ok(input) => input,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let renderer = <$ty as $crate::Renderer>::new();
            let (main, files) = match <$ty as $crate::Renderer>::render(&renderer, input.0) {
                Ok(x) => x,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let files = match $crate::__to_specs(files) {
                Ok(x) => x,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let output = $crate::RendererOutput { main, files };
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
