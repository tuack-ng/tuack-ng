use log::warn;
use quick_xml::de::from_str;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub enum JudgeResult {
    /// 标准结果类型
    Accepted,
    WrongAnswer,
    PresentationError,
    Fail,
    /// 部分正确或得分结果，统一表示为 0-100 的分数
    Score(f64),
}

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

pub fn parse_result(xml_str: &str) -> Result<(JudgeResult, String), Box<dyn std::error::Error>> {
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
        other => return Err(format!("Unknown outcome type: {}", other).into()),
    };

    Ok((result, message))
}

fn parse_score_value(attr_value: &Option<String>) -> Result<f64, Box<dyn std::error::Error>> {
    //从属性获取
    if let Some(value_str) = attr_value {
        return value_str.parse().map(normalize_score).map_err(|e| e.into());
    }

    warn!("缺失分数字段");
    // 默认返回0分
    Ok(0.0)
}

/// 标准化分数到0-100范围
fn normalize_score(score: f64) -> f64 {
    if score > 100.0 {
        100.0
    } else if score < 0.0 {
        0.0
    } else {
        score
    }
}
