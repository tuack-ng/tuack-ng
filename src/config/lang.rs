use crate::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Language {
    /// 语言名称
    pub language: String,
    /// 编译器(None 不编译)
    pub compiler: Option<Compiler>,
    /// 运行器(None 直接运行)
    pub runner: Option<Runner>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Compiler {
    // e.g.: "g++"
    // e.g.: "javac"
    pub executable: String,
    // e.g.: "{executable} --version"
    // e.g.: "{executable} --version"
    pub check: String,
    // e.g: "{executable} -o {output_path}/{program_name}{exe_suffix} {args} {input_path}"
    // e.g: "{executable} -d {output_path}/ {args} {input_path}"
    pub run: String,
    // 可用变量：
    // {executable}：同 executable
    // {output_path}: 输出目录
    // {program_name}：这道题叫啥（也是预期文件名）
    // {args}：用户自定义文件名
    // {input_path}：源文件路径
    // {exe_suffix}：exe后缀名
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Runner {
    // e.g.: "python"
    // e.g.: "java"
    pub executable: String,
    // e.g.: "{executable} --version"
    // e.g.: "{executable} --version"
    pub check: String,
    // e.g: "{executable} {input_path}/{program_name}.py"
    // CP，假定选手使用Main
    // e.g: "{executable} {input_path}/Main.class"
    pub run: String,
    // 可用变量：
    // {executable}：同 executable
    // {input_path}：编译器产物路径（上一步的output_path）如果跳过编译会直接拷贝源文件（保留后缀但名字变成这道题）
    // {program_name}：这道题叫啥（也是预期文件名）
    // {exe_suffix}：exe后缀名
}
