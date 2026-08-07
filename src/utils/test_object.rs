use crate::config::ExpandedDataItem;
use crate::prelude::*;

/// 从字符串解析测试点，返回匹配的 ExpandedDataItem 列表
pub fn parse_test_object(s: &str, all_items: &[ExpandedDataItem]) -> Result<Vec<ExpandedDataItem>> {
    let s = s.trim().to_lowercase();

    if s == "all" {
        return Ok(all_items.to_vec());
    }

    let mut result = Vec::new();
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        if let Some(pos) = part.find('-') {
            let start_str = &part[..pos];
            let end_str = &part[pos + 1..];

            let start = start_str
                .parse::<u32>()
                .with_context(|| format!("无效的起始 ID: {}", start_str))?;
            let end = end_str
                .parse::<u32>()
                .with_context(|| anyhow!("无效的结束 ID: {}", end_str))?;

            if start > end {
                bail!("起始 ID 不能大于结束 ID: {}", part);
            }

            // 遍历查找在范围内的测试点
            for item in all_items.iter() {
                if item.id >= start && item.id <= end {
                    result.push(item.clone());
                }
            }
        } else {
            let id = part
                .parse::<u32>()
                .with_context(|| anyhow!("无效的测试点 ID: {}", part))?;

            // 遍历查找匹配的测试点
            if let Some(item) = all_items.iter().find(|item| item.id == id) {
                result.push(item.clone());
            }
        }
    }

    Ok(result)
}
