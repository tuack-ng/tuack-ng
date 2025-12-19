use super::models::{DateInfo, Problem, SupportLanguage};
use serde::{Deserialize, Serialize};

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
