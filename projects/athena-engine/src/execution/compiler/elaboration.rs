//! 语义 elaboration：实参求值策略（迁出 `lower_pure_expr` 内联表）。
//!
//! Living `04`：哪个算子对第几个参数 Evaluate / Capture，属于 semantic elaboration，
//! 不得散落在 fused CFG lowering 的 `matches!` 里。本模块先收成显式表；
//! 终态由方言 / 前端注入 policy，而不是 Athena 内核猜整包持有求值策略。

use athena_ir::SemanticOperator;

/// 单个实参的求值策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArgumentEvaluationKind {
    /// 递归 elaborate（求值后传入算子）。
    Evaluate,
    /// 捕获为 `LoadTerm`，不求值。
    CaptureAsTerm,
}

/// 解析封闭语义算子在给定 arity / 下标上的实参策略。
///
/// 当前表仍是迁移期内核默认值（历史 evaluator 属性的显式化）。
/// 禁止在 `lower_pure_expr` 外再复制一份 `Hold`/`Sum` 特例。
pub fn argument_evaluation_for_semantic(operator: SemanticOperator, arg_count: usize, index: usize) -> ArgumentEvaluationKind {
    if matches!(operator, SemanticOperator::Hold | SemanticOperator::Function) {
        return ArgumentEvaluationKind::CaptureAsTerm;
    }
    if index == 0
        && (operator == SemanticOperator::Product
            || (operator == SemanticOperator::Sum && arg_count == 2)
            || matches!(operator, SemanticOperator::Apply | SemanticOperator::Map))
    {
        return ArgumentEvaluationKind::CaptureAsTerm;
    }
    if index == 1 && matches!(operator, SemanticOperator::CollectMatches | SemanticOperator::Matches) && arg_count >= 2 {
        return ArgumentEvaluationKind::CaptureAsTerm;
    }
    ArgumentEvaluationKind::Evaluate
}
