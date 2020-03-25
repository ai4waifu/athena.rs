//! [`super::ExecutionCompiler`] 的 Define / 绑定 lowering。

use athena_ir::{Atom, TermNode};
use athena_types::{BindingEvaluationPolicy, BindingKind, Diagnostic, DiagnosticCode, Result, TermId};

use super::{ExecutionCompiler, ModuleBuilder};
use crate::{
    api::request::{AthenaRequest, ControlPlan, SessionCommand},
    execution::ir::{
        BasicBlock, BlockEdge, BlockId, ConstantValue, EffectKind, ExecutionValueType, Operation, OperationKind, SsaValueId, Terminator,
    },
    runtime::session::Session,
};

impl ExecutionCompiler {
    /// 捕获 pattern 左部 + 残差右部，再 `RegisterRuleDispatch`。
    pub(crate) fn lower_register_rule(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        symbol: athena_types::SymbolId,
        pattern: TermId,
        value: TermId,
    ) -> Result<SsaValueId> {
        let Some(name) = session.arena.symbols().resolve(symbol)
        else {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("status", "rule_head_symbol_unresolved"));
        };
        let operator = session.extensions.intern(name);
        let key = builder.ssa();
        let key_constant = builder.push_constant(ConstantValue::symbol(symbol));
        let pattern_root = builder.push_term_root(pattern);
        let pattern_ssa = builder.ssa();
        let value_root = builder.push_term_root(value);
        let value_ssa = builder.ssa();
        let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
        let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
        let unit = builder.ssa();
        blocks.push(BasicBlock {
            id: block_id,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    result: Some(key),
                    result_type: ExecutionValueType::Symbol,
                    kind: OperationKind::Constant { constant: key_constant },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(pattern_ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::LoadTerm { root: pattern_root },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(value_ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::LoadTerm { root: value_root },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(unit),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::RegisterRuleDispatch { head: key, operator, pattern: pattern_ssa, replacement: value_ssa },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                },
            ],
            terminator: Terminator::return_value(unit),
        });
        Ok(unit)
    }

    /// 将右部以 `LoadTerm` 捕获，再 `WriteBinding`（原子 / Deferred 复合项）。
    pub(crate) fn lower_define_capture(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        symbol: athena_types::SymbolId,
        value: TermId,
        evaluation: BindingEvaluationPolicy,
    ) -> Result<SsaValueId> {
        let _ = session;
        let key = builder.ssa();
        let key_constant = builder.push_constant(ConstantValue::symbol(symbol));
        let root = builder.push_term_root(value);
        let rhs = builder.ssa();
        let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
        let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
        let unit = builder.ssa();
        let residual = !matches!(evaluation, BindingEvaluationPolicy::EvaluateBeforeStore);
        let returned = if residual { unit } else { rhs };
        blocks.push(BasicBlock {
            id: block_id,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    result: Some(key),
                    result_type: ExecutionValueType::Symbol,
                    kind: OperationKind::Constant { constant: key_constant },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(rhs),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::LoadTerm { root },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(unit),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::WriteBinding { key, value: rhs, kind: BindingKind::Session, evaluation },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                },
            ],
            terminator: Terminator::return_value(returned),
        });
        Ok(returned)
    }

    /// 复合右部的立即 `Define`：先求值再绑定结果。
    pub(crate) fn lower_define_evaluated(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        symbol: athena_types::SymbolId,
        rhs: TermId,
    ) -> Result<SsaValueId> {
        let eval_block = builder.block_id();
        let bind_block = builder.block_id();
        let entry_cond = builder.ssa();
        let entry_true = builder.push_constant(ConstantValue::boolean(true));

        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(entry_cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: entry_true },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::Branch {
                condition: entry_cond,
                then_edge: BlockEdge::jump(eval_block),
                else_edge: BlockEdge::jump(eval_block),
            },
        });

        let rhs_value = self.lower_request(session, builder, blocks, eval_block, &AthenaRequest::Term(rhs))?;
        self.rewrite_returns_to_join(builder, blocks, bind_block, rhs_value)?;

        let value_param = builder.ssa();
        let key = builder.ssa();
        let key_constant = builder.push_constant(ConstantValue::symbol(symbol));
        let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
        let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
        let unit = builder.ssa();
        blocks.push(BasicBlock {
            id: bind_block,
            parameters: vec![crate::execution::ir::BlockParameter { value: value_param, ty: ExecutionValueType::Term }],
            operations: vec![
                Operation {
                    result: Some(key),
                    result_type: ExecutionValueType::Symbol,
                    kind: OperationKind::Constant { constant: key_constant },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(unit),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::WriteBinding {
                        key,
                        value: value_param,
                        kind: BindingKind::Session,
                        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
                    },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                },
            ],
            terminator: Terminator::return_value(value_param),
        });
        Ok(value_param)
    }
}
