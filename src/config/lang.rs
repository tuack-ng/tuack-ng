use serde::{Deserialize, Serialize};

// #[derive(Debug, Serialize, Deserialize)]
// pub struct Languages {
//     pub languages: HashMap<String, Language>,
// }

#[derive(Debug, Serialize, Deserialize)]
pub struct Language {
    /// 语言名称
    pub language: String,
    // 语言简称
    // pub simple_name: String,
    /// 编译器
    pub compiler: Compiler,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Compiler {
    /// 编译器的可执行文件名称
    pub executable: String,
    /// 检查版本的命令行参数
    pub version_check: String,
    /// 设置输出文件的参数
    pub object_set_arg: String,
}
