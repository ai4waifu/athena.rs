//! 实参求值策略表单元测试。

use athena_engine::execution::compiler::{ArgumentEvaluationKind, argument_evaluation_for_semantic};
use athena_ir::SemanticOperator;

#[test]
fn hold_and_function_capture_all_args() {
    for op in [
        SemanticOperator::Hold,
        SemanticOperator::HoldComplete,
        SemanticOperator::Unevaluated,
        SemanticOperator::Timing,
        SemanticOperator::Trace,
        SemanticOperator::ParallelEvaluate,
        SemanticOperator::InputForm,
        SemanticOperator::Cancel,
        SemanticOperator::Function,
    ] {
        assert_eq!(argument_evaluation_for_semantic(op, 2, 0), ArgumentEvaluationKind::CaptureAsTerm);
        assert_eq!(argument_evaluation_for_semantic(op, 2, 1), ArgumentEvaluationKind::CaptureAsTerm);
    }
}

#[test]
fn iterator_sum_and_product_capture_first() {
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::Sum, 2, 0), ArgumentEvaluationKind::CaptureAsTerm);
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::Sum, 2, 1), ArgumentEvaluationKind::Evaluate);
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::Product, 2, 0), ArgumentEvaluationKind::CaptureAsTerm);
}

#[test]
fn unary_sum_evaluates_arg() {
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::Sum, 1, 0), ArgumentEvaluationKind::Evaluate);
}

#[test]
fn matches_captures_second_arg() {
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::Matches, 2, 0), ArgumentEvaluationKind::Evaluate);
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::Matches, 2, 1), ArgumentEvaluationKind::CaptureAsTerm);
}

#[test]
fn rule_evaluates_rhs_rule_deferred_captures_both() {
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::Rule, 2, 0), ArgumentEvaluationKind::CaptureAsTerm);
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::Rule, 2, 1), ArgumentEvaluationKind::Evaluate);
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::RuleDeferred, 2, 0), ArgumentEvaluationKind::CaptureAsTerm);
    assert_eq!(argument_evaluation_for_semantic(SemanticOperator::RuleDeferred, 2, 1), ArgumentEvaluationKind::CaptureAsTerm);
}
