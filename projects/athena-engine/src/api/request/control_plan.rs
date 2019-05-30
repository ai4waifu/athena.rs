//! 控制流与作用域执行计划（中性语义）。

use athena_types::TermId;

use super::AthenaRequest;

/// 控制流计划。分支、循环、作用域执行走此路径，不得只压成普通符号应用。
#[derive(Debug, PartialEq)]
pub enum ControlPlan {
    /// 顺序执行若干子请求，结果取最后一项（空序列无值）。
    Sequence {
        /// 子请求。
        steps: Vec<AthenaRequest>,
    },
    /// 条件分支。
    Branch {
        /// 条件 term（求值后须为布尔语义）。
        condition: TermId,
        /// 真分支。
        then_branch: Box<AthenaRequest>,
        /// 假分支（可缺省）。
        else_branch: Option<Box<AthenaRequest>>,
    },
    /// 词法作用域执行（中性名，不叫 Module）。
    LexicalScope {
        /// 局部符号绑定（符号名由 lowering 已解析为 `TermId` 侧定义命令或预绑定表）。
        body: Box<AthenaRequest>,
    },
    /// 动态作用域执行（中性名，不叫 Block）。
    DynamicScope {
        /// 主体。
        body: Box<AthenaRequest>,
    },
}
