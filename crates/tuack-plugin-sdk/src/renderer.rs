//! 渲染器插件：trait 与注册宏。

use std::path::{Path, PathBuf};

use extism_pdk::Error;
use tuack_lib::ren::RenderDocument;
use tuack_lib::utils::output::OutputFile;

/// 插件渲染器：实现它即可接入 extism。
///
/// `render` 接收可序列化的渲染文档，返回主产物相对路径与产物文件列表
/// （与宿主侧 `tuack_lib::ren::Renderer` 同形）；SDK 负责把文件列表落盘到 `/out`。
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
            if let Err(e) = $crate::__write_outputs(files) {
                $crate::__report_error(&e);
                return -1;
            }
            let output = $crate::RendererOutput { main };
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

/// 将产物文件列表落盘到 `/out`（供 [`renderer`] 宏内部使用）。
#[doc(hidden)]
pub fn __write_outputs(files: Vec<OutputFile>) -> Result<(), Error> {
    let out = Path::new("/out");
    for file in files {
        match file {
            OutputFile::File { path, mut bytes } => {
                let dest = out.join(&path);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut f = std::fs::File::create(&dest)?;
                std::io::copy(&mut bytes, &mut f)?;
            }
            OutputFile::Dir(path) => {
                std::fs::create_dir_all(out.join(&path))?;
            }
        }
    }
    Ok(())
}
