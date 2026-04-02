use std::time::Duration;

pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();

    if secs < 1.0 {
        // 小于 1s，显示毫秒
        let millis = duration.as_millis();
        format!("{:.2}ms", millis as f64)
    } else {
        // 大于等于 1s，显示秒
        format!("{:.3}s", secs)
    }
}
