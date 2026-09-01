use tempfile::NamedTempFile;

use crate::prelude::*;
use quick_xml::de::from_str;
use tuack_lib::data::AsyncReader;
pub use tuack_lib::utils::testlib::JudgeResult;

#[derive(Debug, Deserialize, PartialEq)]
struct XmlResult {
    #[serde(rename = "@outcome")]
    outcome: String,
    #[serde(rename = "@pctype", default)]
    pctype: Option<String>,
    #[serde(rename = "@points", default)]
    points: Option<String>,
    #[serde(rename = "$text")]
    text: Option<String>,
}

/// 解析 testlib checker 的 XML 结果
pub fn parse_result(xml_str: &str) -> Result<(JudgeResult, String)> {
    let xml_str = xml_str.trim();
    let xml_result: XmlResult = from_str(xml_str)?;

    let message = xml_result.text.unwrap_or_default();

    let result = match xml_result.outcome.as_str() {
        "accepted" => JudgeResult::Accepted,
        "wrong-answer" => JudgeResult::WrongAnswer,
        "presentation-error" => JudgeResult::PresentationError,
        "fail" => JudgeResult::Fail,
        "partially-correct" => {
            let score = parse_score_value(&xml_result.pctype)?;
            JudgeResult::Score(score)
        }
        "points" => {
            let score = parse_score_value(&xml_result.points)?;
            JudgeResult::Score(score)
        }
        other => bail!("Unknown outcome type: {}", other),
    };

    Ok((result, message))
}

/// 将输入流流式写入临时文件，供 SPJ 程序按路径读取。
pub async fn write_temp(reader: &mut dyn AsyncReader, prefix: &str) -> Result<NamedTempFile> {
    let tmp = NamedTempFile::with_prefix(prefix)?;
    let mut f = tokio::fs::File::create(tmp.path()).await?;
    tokio::io::copy(reader, &mut f).await?;
    drop(f);
    Ok(tmp)
}

fn parse_score_value(attr_value: &Option<String>) -> Result<f64> {
    if let Some(value_str) = attr_value {
        return value_str
            .parse::<f64>()
            .map(|score| score.clamp(0.0, 100.0))
            .with_context(|| format!("分数解析失败：'{}'", value_str));
    }
    bail!("缺失分数字段");
}
