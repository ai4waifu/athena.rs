//! 模块常量表（编译期已知 · 无字符串）。

use athena_types::{SymbolId, TermId};

/// VM 常量载荷（句柄闭集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmConstant {
    /// 布尔字面量。
    Boolean(bool),
    /// 空单元常量。
    Unit,
    /// `TermStore` 句柄（VM 不拥有 store）。
    Term(TermId),
    /// 绑定键符号句柄。
    Symbol(SymbolId),
}
