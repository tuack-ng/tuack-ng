use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Problem {
    pub name: String,
    pub title: String,
    #[serde(rename = "type")]
    pub problem_type: String,
    pub dir: String,
    pub exec: String,
    pub input: String,
    pub output: String,
    pub time_limit: String,
    pub memory_limit: String,
    pub testcase: String,
    pub point_equal: String,
    pub submit_filename: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupportLanguage {
    pub name: String,
    pub compile_options: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DateInfo {
    pub start: [u32; 6],
    pub end: [u32; 6],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataJson {
    pub title: String,
    pub subtitle: String,
    pub dayname: String,
    pub date: DateInfo,
    pub use_pretest: bool,
    pub noi_style: bool,
    pub file_io: bool,
    pub support_languages: Vec<SupportLanguage>,
    pub problems: Vec<Problem>,
    pub images: Vec<serde_json::Value>,
}
