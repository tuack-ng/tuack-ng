/// 当前工作目录对应的配置层级位置
#[derive(Debug, Clone)]
pub enum CurrentLocation {
    /// 不属于任何配置文件
    None,
    /// 配置文件根目录
    Root,
    /// 比赛日配置文件
    Day(String),
    /// 赛题配置文件
    Problem(String, String),
}
