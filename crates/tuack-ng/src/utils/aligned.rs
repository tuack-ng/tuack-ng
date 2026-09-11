use owo_colors::OwoColorize;
use unicode_width::UnicodeWidthStr;

/// 一组按最大标签宽度对齐的键值行。
#[derive(Default)]
pub struct AlignedFields {
    rows: Vec<(String, String)>,
}

impl AlignedFields {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, label: impl std::fmt::Display, value: impl std::fmt::Display) {
        self.rows.push((label.to_string(), value.to_string()));
    }

    /// 渲染为多行文本：标签灰色、冒号对齐；多行值续行缩进到值起始列。
    pub fn render(&self) -> String {
        let width = self
            .rows
            .iter()
            .map(|(label, _)| label.width())
            .max()
            .unwrap_or(0);
        let indent = " ".repeat(width + 3);
        let mut out = String::new();
        for (label, value) in &self.rows {
            let pad = " ".repeat(width - label.width() + 1);
            let prefix = format!("{label}{pad}:").dimmed().to_string();
            let mut lines = value.lines();
            let first = lines.next().unwrap_or("");
            if first.is_empty() {
                out.push_str(&prefix);
            } else {
                out.push_str(&format!("{prefix} {first}"));
            }
            for line in lines {
                out.push('\n');
                out.push_str(&indent);
                out.push_str(line);
            }
            out.push('\n');
        }
        out.trim_end_matches('\n').to_string()
    }
}
