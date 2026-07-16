/// 计算一个数的以 10 为底的对数的整数部分
///
/// # 参数
/// - `num`: f64 - 要计算的数字
///
/// # 返回值
/// - f64 - 对数的整数部分，特殊情况：
///   - 0 -> -inf
///   - 负数 -> NaN
pub fn int_lg(num: f64) -> f64 {
    if num == 0.0 {
        return f64::NEG_INFINITY;
    }
    if num < 0.0 {
        return f64::NAN;
    }

    let mut n = 0;
    let mut temp = num;

    if temp >= 10.0 {
        while temp >= 10.0 {
            temp /= 10.0;
            n += 1;
        }
    } else if temp < 1.0 {
        while temp < 1.0 {
            temp *= 10.0;
            n -= 1;
        }
    }

    n as f64
}

/// 将整数格式化为带有逗号分隔的字符串
///
/// ## 参数
/// - `num`: i64 - 要格式化的整数
///
/// ## 返回值
/// - String - 格式化后的字符串
pub fn comma(num: i64) -> String {
    if num < 0 {
        return format!("-{}", comma(-num));
    }

    let num_str = num.to_string();
    let len = num_str.len();

    if len <= 3 {
        return num_str;
    }

    let mut result = String::new();

    for (count, ch) in num_str.chars().rev().enumerate() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }

    result.chars().rev().collect()
}

/// 格式化数字为适合阅读的表示形式
///
/// ## 参数
/// - `num`: f64 - 要格式化的数字
/// - `style`: Option<&str> - 格式化样式：
///   - Some("x") - 科学记数法
///   - Some(",") - 逗号分隔形式
///   - None - 自动选择最紧凑的形式
pub fn hn(num: f64, style: Option<&str>) -> String {
    if num == 0.0 {
        return "0".to_string();
    }

    let (neg, abs_num) = if num < 0.0 { ("-", -num) } else { ("", num) };

    if abs_num.fract() == 0.0 {
        let int_num = abs_num as i64;
        return match style {
            Some("x") => {
                let n = int_lg(abs_num) as i32;
                if int_num == 10_i64.pow(n as u32) {
                    format!("{}10^{{{}}}", neg, n)
                } else {
                    format!(
                        "{}{} \\times 10^{{{}}}",
                        neg,
                        int_num / 10_i64.pow(n as u32),
                        n
                    )
                }
            }
            Some(",") => format!("{}{}", neg, comma(int_num)),
            _ => {
                let n = int_lg(abs_num) as i32;
                let scientific_len = if int_num == 10_i64.pow(n as u32) {
                    3 + n.to_string().len()
                } else {
                    let coeff = int_num / 10_i64.pow(n as u32);
                    coeff.to_string().len() + 8 + n.to_string().len()
                };

                let comma_str = comma(int_num);
                let comma_len = comma_str.len() * 4 / 3;

                if comma_len <= scientific_len {
                    format!("{}{}", neg, comma_str)
                } else if int_num == 10_i64.pow(n as u32) {
                    format!("{}10^{{{}}}", neg, n)
                } else {
                    format!(
                        "{}{} \\times 10^{{{}}}",
                        neg,
                        int_num / 10_i64.pow(n as u32),
                        n
                    )
                }
            }
        };
    }

    match style {
        Some("x") | None => {
            let n = int_lg(abs_num) as i32;
            let coeff = abs_num / 10_f64.powi(n);
            format!("{}{} \\times 10^{{{}}}", neg, coeff, n)
        }
        Some(",") => format!("{}{}", neg, abs_num),
        _ => format!("{}{}", neg, abs_num),
    }
}

/// 将数字范围转换为紧凑的表示形式
///
/// ## 参数
/// - `items`: &[i32] - 有序的数字列表
///
/// ## 返回值
/// - String - 格式化后的范围字符串，以$开头和结尾
pub fn cases(items: &[i32]) -> String {
    if items.is_empty() {
        return "$$".to_string();
    }

    let mut result = Vec::new();
    let mut start = items[0];
    let mut end = items[0];

    for &num in &items[1..] {
        if num == end + 1 {
            end = num;
        } else {
            if start == end {
                result.push(format!("{}", start));
            } else if start + 1 == end {
                result.push(format!("{}", start));
                result.push(format!("{}", end));
            } else {
                result.push(format!("{} \\sim {}", start, end));
            }
            start = num;
            end = num;
        }
    }

    if start == end {
        result.push(format!("{}", start));
    } else if start + 1 == end {
        result.push(format!("{}", start));
        result.push(format!("{}", end));
    } else {
        result.push(format!("{} \\sim {}", start, end));
    }

    format!("${}$", result.join(","))
}
