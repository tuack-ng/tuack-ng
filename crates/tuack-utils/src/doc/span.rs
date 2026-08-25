//! span（字节区间）-> 行列 换算工具。

/// 根据源码计算字节偏移对应的行号（1 起）与列号（1 起）。
pub fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut line_start = 0usize;
    for (i, b) in source.bytes().enumerate() {
        if i >= offset {
            break;
        }
        if b == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    // 列按字符数（Unicode 字符）计。
    let col = source[line_start..offset.min(source.len())].chars().count() + 1;
    (line, col)
}

/// 将 `Option<Span>` 转换为 `(Option<line>, Option<col>)`。
pub fn span_to_line_col(
    source: &str,
    span: Option<tuack_ng_parser::Span>,
) -> (Option<usize>, Option<usize>) {
    match span {
        Some(s) => {
            let (line, col) = offset_to_line_col(source, s.start);
            (Some(line), Some(col))
        }
        None => (None, None),
    }
}

/// 计算 1 起行号对应的整行字节区间 `[行首，行尾)`（不含换行符）。
pub fn line_to_byte_span(source: &str, line: usize) -> Option<tuack_ng_parser::Span> {
    if line == 0 {
        return None;
    }
    let mut cur = 1;
    let mut start = 0usize;
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            if cur == line {
                return Some(tuack_ng_parser::Span::new(start, i));
            }
            cur += 1;
            start = i + 1;
        }
    }
    if cur == line {
        Some(tuack_ng_parser::Span::new(start, source.len()))
    } else {
        None
    }
}

/// 将 span 起点拓展到其所在行的行尾（不含换行符）。
pub fn extend_to_line_end(
    source: &str,
    span: Option<tuack_ng_parser::Span>,
) -> Option<tuack_ng_parser::Span> {
    let s = span?;
    let start = s.start.min(source.len());
    let stop = source[start..]
        .find('\n')
        .map(|i| start + i)
        .unwrap_or(source.len());
    Some(tuack_ng_parser::Span::new(start, stop))
}
