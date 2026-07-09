use crate::prelude::*;

/// 数据生成器参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    Integer(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

/// 数据生成器
pub trait Generator: Send {
    /// 准备生成器使其可供使用（编译等）
    fn prepare(&mut self) -> Result<()>;
    /// 运行生成器
    ///
    /// # 参数
    /// `args`: 传给生成器的参数（-key=value）
    /// `seed`: 随机种子
    ///
    /// # 返回值
    /// `Ok(Vec<u8>)`: 如果生成成功，返回生成内容
    /// `Err`: 如果生成失败
    fn run(&self, args: HashMap<String, Arg>, seed: u64) -> Result<Vec<u8>>;
}
