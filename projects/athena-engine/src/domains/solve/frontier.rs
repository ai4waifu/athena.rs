//! 可恢复求解前沿（操作性状态，非数学事实）。

/// 机器可读的恢复前沿种类（封闭枚举 · 非用户文案 · 非 M-Graph relation label）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResumeKind {
    /// 通用截断 / 预算切断。
    Cut,
    /// 线性精确求解未完成。
    LinearExact,
    /// 线性数值 / 机器精度路径未完成。
    LinearMachine,
    /// 一元因式分解未完成。
    UnivariateFactor,
}

/// 恢复令牌：待展开分支、未完成量词块、迭代态或 portfolio 状态。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResumeToken {
    /// 前沿种类（操作性，不得参与语义 fingerprint / admission）。
    pub kind: ResumeKind,
    /// 不透明载荷版本。
    pub version: u16,
    /// 不透明字节（provider 私有编码；admission 前不得当作证明）。
    pub payload: Vec<u8>,
}

impl ResumeToken {
    /// 空载荷前沿。
    pub fn empty(kind: ResumeKind) -> Self {
        Self { kind, version: 0, payload: Vec::new() }
    }
}
