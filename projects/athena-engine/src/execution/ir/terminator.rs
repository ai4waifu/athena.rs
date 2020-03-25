//! 显式块终结器（无隐式指令指针）。

use super::ids::{BlockId, ExitId, SsaValueId};

/// 带块参数的后继边（经块参数实现 SSA phi）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEdge {
    /// 目标块。
    pub target: BlockId,
    /// 传入目标块参数的实参。
    pub arguments: Vec<SsaValueId>,
}

/// 封闭的终结器集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// 在类型化 Boolean SSA 值上的条件分支。
    Branch {
        /// 谓词。
        condition: SsaValueId,
        /// 谓词为真时走此边。
        then_edge: BlockEdge,
        /// 谓词为假时走此边。
        else_edge: BlockEdge,
    },
    /// 在离散判别值上的多路分支。
    Switch {
        /// 判别 SSA 值。
        discriminant: SsaValueId,
        /// `(case 值下标 → 边)` 表（精确编码由编译器填入）。
        cases: Vec<(u32, BlockEdge)>,
        /// 无 case 匹配时的默认边。
        default: BlockEdge,
    },
    /// 从当前 region / module 成功返回。
    Return {
        /// 返回的 SSA 值（顺序由 region 签名固定）。
        values: Vec<SsaValueId>,
    },
    /// 硬拒绝，走已声明出口 / 诊断路径。
    Reject {
        /// 可选的 module 出口描述符。
        exit: Option<ExitId>,
    },
    /// 将控制交还运行时 / provider（不是调度器）。
    Yield {
        /// 交给运行时上下文的值。
        values: Vec<SsaValueId>,
        /// Yield 完成后的恢复边。
        resume: BlockEdge,
    },
    /// 不可达标记，供校验器完备性使用。
    Unreachable,
}

impl BlockEdge {
    /// 无块参数的边。
    pub fn jump(target: BlockId) -> Self {
        Self { target, arguments: Vec::new() }
    }
}

impl Terminator {
    /// 返回单个值的简单返回。
    pub fn return_value(value: SsaValueId) -> Self {
        Self::Return { values: vec![value] }
    }
}
