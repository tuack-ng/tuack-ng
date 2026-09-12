//! 导出器插件：trait 与注册宏。

use extism_pdk::Error;
use tuack_lib::dump::DumpDocument;
use tuack_lib::utils::output::OutputFile;

/// 插件导出器：实现它即可接入 extism。
///
/// `dump` 接收可序列化的导出文档，返回产物文件列表与导出警告
/// （与宿主侧 `tuack_lib::dump::Dumper` 同形）；SDK 把文件列表转成可回传的
/// [`OutputSpec`](crate::OutputSpec)，由宿主落盘。
pub trait Dumper: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    fn dump(&self, doc: DumpDocument) -> Result<(Vec<OutputFile>, Vec<String>), Error>;
}

/// 注册导出器为 extism 导出函数（默认导出名 `dump`）。
#[macro_export]
macro_rules! dumper {
    ($ty:ty) => {
        $crate::dumper!($ty, dump);
    };
    ($ty:ty, $name:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name() -> i32 {
            $crate::__init_logger();
            let input: $crate::Json<$crate::DumpDocument> = match $crate::extism_pdk::input() {
                Ok(input) => input,
                Err(e) => {
                    $crate::__report_error(&e);
                    return -1;
                }
            };
            let dumper = <$ty as $crate::Dumper>::new();
            let (files, warnings) = match <$ty as $crate::Dumper>::dump(&dumper, input.0) {
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
            let output = $crate::DumperOutput { warnings, files };
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
