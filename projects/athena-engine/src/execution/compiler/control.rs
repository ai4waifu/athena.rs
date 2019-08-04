//! Control-plan and scope lowering for [`super::ExecutionCompiler`].

use athena_ir::{Atom, TermNode};
use athena_types::{BindingEvaluationPolicy, BindingKind, Diagnostic, DiagnosticCode, Result, TermId};

use super::{ExecutionCompiler, ModuleBuilder};
use super::helpers::expand_span_range;
use crate::{
    api::request::{AthenaRequest, ControlPlan, SessionCommand},
    execution::ir::{
        BasicBlock, BlockEdge, BlockId, ConstantValue, EffectKind, ExecutionValueType, Operation, OperationKind, SsaValueId, Terminator,
    },
    runtime::session::Session,
};

impl ExecutionCompiler {
    pub(crate) fn lower_control(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        plan: &ControlPlan,
    ) -> Result<SsaValueId> {
        match plan {
            ControlPlan::Sequence { steps } => self.lower_sequence(session, builder, blocks, block_id, steps),
            ControlPlan::Branch { condition, then_branch, else_branch } => {
                self.lower_branch(session, builder, blocks, block_id, *condition, then_branch, else_branch.as_deref())
            }
            ControlPlan::LocalScope { body } | ControlPlan::LexicalScope { body } | ControlPlan::DynamicScope { body } => {
                self.lower_scope(session, builder, blocks, block_id, body)
            }
            ControlPlan::Cond { arms, otherwise } => self.lower_cond(session, builder, blocks, block_id, arms, otherwise.as_deref()),
            ControlPlan::Recover { body, handler } => self.lower_recover(session, builder, blocks, block_id, body, handler),
            ControlPlan::LoopWhile { condition, body } => self.lower_loop_while(session, builder, blocks, block_id, *condition, body),
            ControlPlan::CountedLoop { variable, iterator, body } => {
                self.lower_counted_loop(session, builder, blocks, block_id, *variable, *iterator, body)
            }
            ControlPlan::Iterate { binder, range, body, evaluation: _ } => {
                self.lower_counted_loop(session, builder, blocks, block_id, *binder, *range, body)
            }
        }
    }

    pub(crate) fn lower_counted_loop(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        variable: TermId,
        iterator: TermId,
        body: &AthenaRequest,
    ) -> Result<SsaValueId> {
        let symbol = self.require_symbol_atom(session, variable)?;
        let items = self.require_atom_list(session, iterator)?;
        let AthenaRequest::Term(body_term) = body
        else {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "counted_loop_body_must_be_term"));
        };

        if items.is_empty() {
            let value = builder.ssa();
            let constant = builder.push_constant(ConstantValue::Unit);
            blocks.push(BasicBlock {
                id: entry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    result: Some(value),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::Constant { constant },
                    effect_in: None,
                    effect_out: None,
                }],
                terminator: Terminator::return_value(value),
            });
            return Ok(value);
        }

        // Bootstrap: unroll constant atom lists into Define + body Term steps.
        let mut steps = Vec::with_capacity(items.len().saturating_mul(2));
        for item in items {
            steps.push(AthenaRequest::Command(SessionCommand::Define {
                symbol,
                value: item,
                kind: BindingKind::Session,
                evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
            }));
            steps.push(AthenaRequest::Term(*body_term));
        }
        let budget_in = builder.push_effect(EffectKind::BudgetCheck, None);
        let _budget_out = builder.push_effect(EffectKind::BudgetCheck, Some(budget_in));
        self.lower_sequence(session, builder, blocks, entry, &steps)
    }

    pub(crate) fn require_symbol_atom(&self, session: &mut Session, term: TermId) -> Result<athena_types::SymbolId> {
        match session.arena.get(term) {
            Some(TermNode::Atom(Atom::Symbol(symbol))) => Ok(*symbol),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "counted_loop_variable_not_symbol")),
            None => {
                Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("component", "ExecutionCompiler").detail("reason", "missing_term"))
            }
        }
    }

    pub(crate) fn require_atom_list(&self, session: &mut Session, term: TermId) -> Result<Vec<TermId>> {
        let list_items = match session.arena.get(term) {
            Some(TermNode::List(items)) => Some(items.clone()),
            _ => None,
        };
        if let Some(items) = list_items {
            for item in &items {
                self.require_atom(session, *item)?;
            }
            return Ok(items);
        }
        let span_args = match session.arena.get(term) {
            Some(TermNode::Application { head, arguments }) if session.operators.name(*head) == Some("Span") => Some(arguments.clone()),
            _ => None,
        };
        if let Some(arguments) = span_args {
            return self.expand_span_iterator(session, &arguments);
        }
        match session.arena.get(term) {
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "counted_loop_iterator_not_atom_list")),
            None => {
                Err(Diagnostic::new(DiagnosticCode::InvalidIndex).detail("component", "ExecutionCompiler").detail("reason", "missing_term"))
            }
        }
    }

    pub(crate) fn expand_span_iterator(&self, session: &mut Session, arguments: &[TermId]) -> Result<Vec<TermId>> {
        let ints: Option<Vec<i64>> = arguments
            .iter()
            .map(|t| match session.arena.get(*t) {
                Some(TermNode::Atom(Atom::Number(n))) => n.as_exact_integer(),
                _ => None,
            })
            .collect();
        let Some(ints) = ints
        else {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "span_bounds_not_integer"));
        };
        let values = match ints.as_slice() {
            [a, b] => expand_span_range(*a, 1, *b),
            [a, step, b] => expand_span_range(*a, *step, *b),
            _ => {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionCompiler")
                    .detail("status", "span_arity_not_supported"));
            }
        };
        let Some(values) = values
        else {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "span_range_invalid"));
        };
        Ok(values.into_iter().map(|v| session.builder().int(v, Default::default())).collect())
    }

    pub(crate) fn lower_loop_while(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        condition: TermId,
        body: &AthenaRequest,
    ) -> Result<SsaValueId> {
        let header = builder.block_id();
        let body_block = builder.block_id();
        let exit = builder.block_id();
        let acc_param = builder.ssa();
        let exit_param = builder.ssa();
        let init = builder.ssa();
        let init_const = builder.push_constant(ConstantValue::Unit);
        let entry_cond = builder.ssa();
        let entry_true = builder.push_constant(ConstantValue::boolean(true));
        let budget_in = builder.push_effect(EffectKind::BudgetCheck, None);
        let budget_out = builder.push_effect(EffectKind::BudgetCheck, Some(budget_in));

        // entry → header(Unit)
        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    result: Some(init),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::Constant { constant: init_const },
                    effect_in: None,
                    effect_out: None,
                },
                Operation {
                    result: Some(entry_cond),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: entry_true },
                    effect_in: Some(budget_in),
                    effect_out: Some(budget_out),
                },
            ],
            terminator: Terminator::Branch {
                condition: entry_cond,
                then_edge: BlockEdge { target: header, arguments: vec![init] },
                else_edge: BlockEdge { target: header, arguments: vec![init] },
            },
        });

        // Header re-evaluates the predicate each iteration (ReadBinding / compares).
        let mut header_ops = Vec::new();
        let loop_cond = match self.require_boolean_atom(session, condition) {
            Ok(cond_bool) => {
                let loop_cond = builder.ssa();
                let loop_const = builder.push_constant(ConstantValue::boolean(cond_bool));
                header_ops.push(Operation {
                    result: Some(loop_cond),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: loop_const },
                    effect_in: None,
                    effect_out: None,
                });
                loop_cond
            }
            Err(_) => self.lower_pure_expr(session, builder, &mut header_ops, condition)?,
        };
        blocks.push(BasicBlock {
            id: header,
            parameters: vec![crate::execution::ir::BlockParameter { value: acc_param, ty: ExecutionValueType::Term }],
            operations: header_ops,
            terminator: Terminator::Branch {
                condition: loop_cond,
                then_edge: BlockEdge::jump(body_block),
                else_edge: BlockEdge { target: exit, arguments: vec![acc_param] },
            },
        });

        let body_value = self.lower_request(session, builder, blocks, body_block, body)?;
        // Body returns continue at header with the new accumulator.
        self.rewrite_returns_to_join(builder, blocks, header, body_value)?;

        blocks.push(BasicBlock {
            id: exit,
            parameters: vec![crate::execution::ir::BlockParameter { value: exit_param, ty: ExecutionValueType::Term }],
            operations: Vec::new(),
            terminator: Terminator::return_value(exit_param),
        });
        Ok(exit_param)
    }

    pub(crate) fn lower_recover(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        body: &AthenaRequest,
        handler: &AthenaRequest,
    ) -> Result<SsaValueId> {
        let body_block = builder.block_id();
        let handler_block = builder.block_id();
        let join = builder.block_id();
        let result_param = builder.ssa();
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
                then_edge: BlockEdge::jump(body_block),
                else_edge: BlockEdge::jump(body_block),
            },
        });

        let body_value = self.lower_request(session, builder, blocks, body_block, body)?;
        self.rewrite_rejects_to_handler(builder, blocks, handler_block)?;
        self.rewrite_returns_to_join(builder, blocks, join, body_value)?;

        let handler_value = self.lower_request(session, builder, blocks, handler_block, handler)?;
        self.rewrite_returns_to_join(builder, blocks, join, handler_value)?;

        blocks.push(BasicBlock {
            id: join,
            parameters: vec![crate::execution::ir::BlockParameter { value: result_param, ty: ExecutionValueType::Term }],
            operations: Vec::new(),
            terminator: Terminator::return_value(result_param),
        });
        Ok(result_param)
    }

    pub(crate) fn rewrite_rejects_to_handler(&self, builder: &mut ModuleBuilder, blocks: &mut Vec<BasicBlock>, handler: BlockId) -> Result<()> {
        let reject_ids: Vec<BlockId> = blocks.iter().filter(|b| matches!(b.terminator, Terminator::Reject { .. })).map(|b| b.id).collect();
        for block_id in reject_ids {
            let cond = builder.ssa();
            let true_const = builder.push_constant(ConstantValue::boolean(true));
            let block = blocks.iter_mut().find(|b| b.id == block_id).expect("block");
            block.operations.push(Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: true_const },
                effect_in: None,
                effect_out: None,
            });
            block.terminator = Terminator::Branch { condition: cond, then_edge: BlockEdge::jump(handler), else_edge: BlockEdge::jump(handler) };
        }
        Ok(())
    }

    pub(crate) fn lower_cond(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        arms: &[(TermId, Box<AthenaRequest>)],
        otherwise: Option<&AthenaRequest>,
    ) -> Result<SsaValueId> {
        if arms.is_empty() {
            return match otherwise {
                Some(request) => self.lower_request(session, builder, blocks, entry, request),
                None => {
                    let value = builder.ssa();
                    let constant = builder.push_constant(ConstantValue::Unit);
                    blocks.push(BasicBlock {
                        id: entry,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            result: Some(value),
                            result_type: ExecutionValueType::Unit,
                            kind: OperationKind::Constant { constant },
                            effect_in: None,
                            effect_out: None,
                        }],
                        terminator: Terminator::return_value(value),
                    });
                    Ok(value)
                }
            };
        }

        let join = builder.block_id();
        let result_param = builder.ssa();
        let mut test_blocks = Vec::with_capacity(arms.len());
        test_blocks.push(entry);
        for _ in 1..arms.len() {
            test_blocks.push(builder.block_id());
        }
        let otherwise_block = builder.block_id();

        for (index, (condition, arm)) in arms.iter().enumerate() {
            let mut operations = Vec::new();
            let cond_value = match self.require_boolean_atom(session, *condition) {
                Ok(cond_bool) => {
                    let cond_value = builder.ssa();
                    let cond_constant = builder.push_constant(ConstantValue::boolean(cond_bool));
                    operations.push(Operation {
                        result: Some(cond_value),
                        result_type: ExecutionValueType::Boolean,
                        kind: OperationKind::Constant { constant: cond_constant },
                        effect_in: None,
                        effect_out: None,
                    });
                    cond_value
                }
                Err(_) => self.lower_pure_expr(session, builder, &mut operations, *condition)?,
            };
            let arm_block = builder.block_id();
            let else_target = if index + 1 < arms.len() { test_blocks[index + 1] } else { otherwise_block };
            blocks.push(BasicBlock {
                id: test_blocks[index],
                parameters: Vec::new(),
                operations,
                terminator: Terminator::Branch {
                    condition: cond_value,
                    then_edge: BlockEdge::jump(arm_block),
                    else_edge: BlockEdge::jump(else_target),
                },
            });
            let arm_value = self.lower_request(session, builder, blocks, arm_block, arm)?;
            self.rewrite_returns_to_join(builder, blocks, join, arm_value)?;
        }

        match otherwise {
            Some(request) => {
                let other_value = self.lower_request(session, builder, blocks, otherwise_block, request)?;
                self.rewrite_returns_to_join(builder, blocks, join, other_value)?;
            }
            None => {
                let value = builder.ssa();
                let constant = builder.push_constant(ConstantValue::Unit);
                blocks.push(BasicBlock {
                    id: otherwise_block,
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        result: Some(value),
                        result_type: ExecutionValueType::Unit,
                        kind: OperationKind::Constant { constant },
                        effect_in: None,
                        effect_out: None,
                    }],
                    terminator: Terminator::Return { values: vec![value] },
                });
                self.rewrite_returns_to_join(builder, blocks, join, value)?;
            }
        }

        blocks.push(BasicBlock {
            id: join,
            parameters: vec![crate::execution::ir::BlockParameter { value: result_param, ty: ExecutionValueType::Term }],
            operations: Vec::new(),
            terminator: Terminator::return_value(result_param),
        });
        Ok(result_param)
    }

    /// Rewrite current `Return` terminators into jumps to `join` with a block argument.
    pub(crate) fn rewrite_returns_to_join(
        &self,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        join: BlockId,
        fallback: SsaValueId,
    ) -> Result<()> {
        let return_block_ids: Vec<BlockId> =
            blocks.iter().filter(|b| matches!(b.terminator, Terminator::Return { .. })).map(|b| b.id).collect();
        for block_id in return_block_ids {
            let forwarded = {
                let block = blocks.iter().find(|b| b.id == block_id).expect("block");
                match &block.terminator {
                    Terminator::Return { values } => values.first().copied().unwrap_or(fallback),
                    _ => fallback,
                }
            };
            let cond = builder.ssa();
            let true_const = builder.push_constant(ConstantValue::boolean(true));
            let block = blocks.iter_mut().find(|b| b.id == block_id).expect("block");
            block.operations.push(Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: true_const },
                effect_in: None,
                effect_out: None,
            });
            block.terminator = Terminator::Branch {
                condition: cond,
                then_edge: BlockEdge { target: join, arguments: vec![forwarded] },
                else_edge: BlockEdge { target: join, arguments: vec![forwarded] },
            };
        }
        Ok(())
    }

    pub(crate) fn lower_scope(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        body: &AthenaRequest,
    ) -> Result<SsaValueId> {
        let body_block = builder.block_id();
        let exit_block = builder.block_id();
        let scope = builder.ssa();
        let enter_in = builder.push_effect(EffectKind::EnterScope, None);
        let enter_out = builder.push_effect(EffectKind::EnterScope, Some(enter_in));
        let entry_cond = builder.ssa();
        let entry_true = builder.push_constant(ConstantValue::boolean(true));

        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    result: Some(scope),
                    result_type: ExecutionValueType::Scope,
                    kind: OperationKind::EnterScope { parent: None },
                    effect_in: Some(enter_in),
                    effect_out: Some(enter_out),
                },
                Operation {
                    result: Some(entry_cond),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: entry_true },
                    effect_in: None,
                    effect_out: None,
                },
            ],
            terminator: Terminator::Branch {
                condition: entry_cond,
                then_edge: BlockEdge::jump(body_block),
                else_edge: BlockEdge::jump(body_block),
            },
        });

        let body_value = self.lower_request(session, builder, blocks, body_block, body)?;
        // Any body Return (including Sequence tails) continues to ExitScope.
        let return_block_ids: Vec<BlockId> =
            blocks.iter().filter(|b| matches!(b.terminator, Terminator::Return { .. })).map(|b| b.id).collect();
        for block_id in return_block_ids {
            let forwarded = {
                let block = blocks.iter().find(|b| b.id == block_id).expect("block");
                match &block.terminator {
                    Terminator::Return { values } => values.first().copied().unwrap_or(body_value),
                    _ => body_value,
                }
            };
            let cond = builder.ssa();
            let true_const = builder.push_constant(ConstantValue::boolean(true));
            let block = blocks.iter_mut().find(|b| b.id == block_id).expect("block");
            block.operations.push(Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: true_const },
                effect_in: None,
                effect_out: None,
            });
            block.terminator = Terminator::Branch {
                condition: cond,
                then_edge: BlockEdge { target: exit_block, arguments: vec![forwarded] },
                else_edge: BlockEdge { target: exit_block, arguments: vec![forwarded] },
            };
        }

        let result_param = builder.ssa();
        let exit_in = builder.push_effect(EffectKind::ExitScope, Some(enter_out));
        let exit_out = builder.push_effect(EffectKind::ExitScope, Some(exit_in));
        blocks.push(BasicBlock {
            id: exit_block,
            parameters: vec![crate::execution::ir::BlockParameter { value: result_param, ty: ExecutionValueType::Term }],
            operations: vec![Operation {
                result: None,
                result_type: ExecutionValueType::Unit,
                kind: OperationKind::ExitScope { scope },
                effect_in: Some(exit_in),
                effect_out: Some(exit_out),
            }],
            terminator: Terminator::return_value(result_param),
        });
        Ok(result_param)
    }

    pub(crate) fn lower_sequence(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        steps: &[AthenaRequest],
    ) -> Result<SsaValueId> {
        if steps.is_empty() {
            let value = builder.ssa();
            let constant = builder.push_constant(ConstantValue::Unit);
            blocks.push(BasicBlock {
                id: entry,
                parameters: Vec::new(),
                operations: vec![Operation {
                    result: Some(value),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::Constant { constant },
                    effect_in: None,
                    effect_out: None,
                }],
                terminator: Terminator::return_value(value),
            });
            return Ok(value);
        }

        let mut step_blocks = Vec::with_capacity(steps.len());
        step_blocks.push(entry);
        for _ in 1..steps.len() {
            step_blocks.push(builder.block_id());
        }

        let mut last_value = SsaValueId(0);
        for (index, step) in steps.iter().enumerate() {
            let block_id = step_blocks[index];
            last_value = self.lower_request(session, builder, blocks, block_id, step)?;
            if index + 1 < steps.len() {
                let next = step_blocks[index + 1];
                // Rewrite every Return produced by this step (including nested eval/bind
                // blocks from Immediate compound `Define`) into a jump to the next step.
                self.rewrite_returns_to_continue(builder, blocks, next)?;
            }
        }
        Ok(last_value)
    }

    /// Chain sequence steps: turn outstanding `Return` terminators into jumps to `next`.
    pub(crate) fn rewrite_returns_to_continue(&self, builder: &mut ModuleBuilder, blocks: &mut Vec<BasicBlock>, next: BlockId) -> Result<()> {
        let return_block_ids: Vec<BlockId> =
            blocks.iter().filter(|b| matches!(b.terminator, Terminator::Return { .. })).map(|b| b.id).collect();
        for block_id in return_block_ids {
            let cond = builder.ssa();
            let true_const = builder.push_constant(ConstantValue::boolean(true));
            let block = blocks.iter_mut().find(|b| b.id == block_id).expect("block");
            block.operations.push(Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: true_const },
                effect_in: None,
                effect_out: None,
            });
            block.terminator = Terminator::Branch { condition: cond, then_edge: BlockEdge::jump(next), else_edge: BlockEdge::jump(next) };
        }
        Ok(())
    }

    pub(crate) fn lower_branch(
        &self,
        session: &mut Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        condition: TermId,
        then_branch: &AthenaRequest,
        else_branch: Option<&AthenaRequest>,
    ) -> Result<SsaValueId> {
        let then_block = builder.block_id();
        let else_block = builder.block_id();
        let mut operations = Vec::new();
        let cond_value = match self.require_boolean_atom(session, condition) {
            Ok(cond_bool) => {
                let cond_value = builder.ssa();
                let cond_constant = builder.push_constant(ConstantValue::boolean(cond_bool));
                operations.push(Operation {
                    result: Some(cond_value),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: cond_constant },
                    effect_in: None,
                    effect_out: None,
                });
                cond_value
            }
            Err(_) => {
                // Runtime predicate: `Equal[...]`, `True`/`False` symbols, numeric truthiness, etc.
                self.lower_pure_expr(session, builder, &mut operations, condition)?
            }
        };

        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations,
            terminator: Terminator::Branch {
                condition: cond_value,
                then_edge: BlockEdge::jump(then_block),
                else_edge: BlockEdge::jump(else_block),
            },
        });

        let then_value = self.lower_request(session, builder, blocks, then_block, then_branch)?;
        match else_branch {
            Some(request) => {
                let _else_value = self.lower_request(session, builder, blocks, else_block, request)?;
                Ok(then_value)
            }
            None => self.lower_unit_else(builder, blocks, else_block, then_value),
        }
    }

    pub(crate) fn lower_unit_else(
        &self,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        else_block: BlockId,
        then_value: SsaValueId,
    ) -> Result<SsaValueId> {
        // Missing else of `If`/`Branch` publishes as `Null` (Unit → Null at result materialization).
        let value = builder.ssa();
        let constant = builder.push_constant(ConstantValue::Unit);
        blocks.push(BasicBlock {
            id: else_block,
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(value),
                result_type: ExecutionValueType::Unit,
                kind: OperationKind::Constant { constant },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::return_value(value),
        });
        Ok(then_value)
    }

}
