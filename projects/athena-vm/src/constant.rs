//! 模块常量表（编译期已知 · 无字符串）。

/// VM 常量载荷（句柄闭集的最小子集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmConstant {
    /// Boolean 字面量。
    Boolean(bool),
    /// Unit。
    Unit,
}
