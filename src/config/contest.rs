use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use super::problem::ProblemConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContestConfig {
    pub version: u32,
    pub folder: String,
    pub name: String,
    pub subdir: Vec<String>,
    pub title: String,
    #[serde(rename = "short title")]
    pub short_title: String,
    #[serde(rename = "use-pretest")]
    #[serde(default)]
    pub use_pretest: Option<bool>,
    #[serde(rename = "noi-style")]
    #[serde(default)]
    pub noi_style: Option<bool>,
    #[serde(rename = "file-io")]
    #[serde(default)]
    pub file_io: Option<bool>,
    #[serde(skip)]
    pub subconfig: Vec<ContestDayConfig>,
    #[serde(skip)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContestDayConfig {
    pub version: u32,
    pub folder: String,
    pub name: String,
    pub subdir: Vec<String>,
    pub title: String,
    pub compile: CompileConfig,
    #[serde(rename = "start time")]
    pub start_time: [u32; 6],
    #[serde(rename = "end time")]
    pub end_time: [u32; 6],
    #[serde(rename = "use-pretest")]
    #[serde(default)]
    pub use_pretest: Option<bool>,
    #[serde(rename = "noi-style")]
    #[serde(default)]
    pub noi_style: Option<bool>,
    #[serde(rename = "file-io")]
    #[serde(default)]
    pub file_io: Option<bool>,
    #[serde(skip)]
    pub subconfig: Vec<ProblemConfig>,
    #[serde(skip)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CompileConfig {
    pub cpp: String,
    #[serde(default)]
    pub c: String,
}