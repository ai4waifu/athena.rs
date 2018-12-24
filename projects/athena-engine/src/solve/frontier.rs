//! 可恢复求解前沿（操作性状态，非数学事实）。

/// 恢复令牌：待展开分支、未完成量词块、迭代态或 portfolio 状态。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResumeToken {
    /// 机器可读前沿标签（稳定，非用户文案）。
    pub label: String,
    /// 不透明载荷版本。
    pub version: u16,
    /// 不透明字节（provider 私有编码；admission 前不得当作证明）。
    pub payload: Vec<u8>,
}

impl ResumeToken {
    /// 空标签前沿。
    pub fn empty(label: impl Into<String>) -> Self {
        Self { label: label.into(), version: 0, payload: Vec::new() }
    }
}
