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
pub struct TemplateManifest {
    #[serde(default = "default_use_pretest")]
    pub use_pretest: bool,
    #[serde(default = "default_noi_style")]
    pub noi_style: bool,
    #[serde(default = "default_file_io")]
    pub file_io: bool,
}

fn default_use_pretest() -> bool {
    false
}

fn default_noi_style() -> bool {
    true
}

fn default_file_io() -> bool {
    true
}
