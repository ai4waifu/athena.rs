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
    /// 条件分支（对齐语义算子 `Branch`）。
    Branch {
        /// 条件 term（求值后须为布尔语义）。
        condition: TermId,
        /// 真分支。
        then_branch: Box<AthenaRequest>,
        /// 假分支（可缺省）。
        else_branch: Option<Box<AthenaRequest>>,
    },
    /// 多条件链（对齐语义算子 `Cond`）。
    Cond {
        /// `(条件, 分支)` 对，按序取首个真条件。
        arms: Vec<(TermId, Box<AthenaRequest>)>,
        /// 可选默认分支。
        otherwise: Option<Box<AthenaRequest>>,
    },
    /// 条件循环（对齐语义算子 `LoopWhile`）。
    LoopWhile {
        /// 循环条件。
        condition: TermId,
        /// 循环体。
        body: Box<AthenaRequest>,
    },
    /// 计数 / 区间循环（对齐语义算子 `CountedLoop`）。
    CountedLoop {
        /// 循环变量（符号项）。
        variable: TermId,
        /// 迭代器项。
        iterator: TermId,
        /// 循环体。
        body: Box<AthenaRequest>,
    },
    /// 中性迭代计划（方言 `Table` / comprehension 等 lowering 目标）。
    Iterate {
        /// 绑定变量（符号项）。
        binder: TermId,
        /// 迭代范围项（已由方言规范化）。
        range: TermId,
        /// 循环体。
        body: Box<AthenaRequest>,
        /// 体求值策略（残余项 vs 逐步求值由 lowering 选定）。
        evaluation: athena_types::BindingEvaluationPolicy,
    },
    /// 错误恢复（对齐语义算子 `Recover`）。
    Recover {
        /// 受保护主体。
        body: Box<AthenaRequest>,
        /// 失败时执行的恢复分支。
        handler: Box<AthenaRequest>,
    },
    /// 立即拒绝当前区域（供 `Recover` 捕获；非 Extension 表面名）。
    Reject,
    /// 代换式局部作用域（对齐语义算子 `LocalScope`）。
    LocalScope {
        /// 主体。
        body: Box<AthenaRequest>,
    },
    /// 词法作用域执行（对齐语义算子 `LexicalScope`）。
    LexicalScope {
        /// 局部符号绑定（符号名由 lowering 已解析为 `TermId` 侧定义命令或预绑定表）。
        body: Box<AthenaRequest>,
    },
    /// 动态作用域执行（对齐语义算子 `DynamicScope`）。
    DynamicScope {
        /// 主体。
        body: Box<AthenaRequest>,
    },
    /// 中性索引（方言 `Part` / 下标 / `end` 等 lowering 目标 · ）。
    Index {
        /// 被索引目标。
        target: TermId,
        /// 各轴 [`athena_types::IndexSpec`]（已由方言规范化）。
        axes: Vec<athena_types::IndexSpec>,
    },
    /// 中性模式测试（方言 `MatchQ` 等 lowering 目标 · ）。
    Match {
        /// 被测项（已物化）。
        target: TermId,
        /// 中性 [`crate::reasoning::trs::TermPattern`]。
        pattern: crate::reasoning::trs::TermPattern,
    },
    /// 中性模式收集（方言 `Cases` 等 lowering 目标 · ）。
    CollectMatches {
        /// 源集合项。
        source: TermId,
        /// 中性模式。
        pattern: crate::reasoning::trs::TermPattern,
    },
}
