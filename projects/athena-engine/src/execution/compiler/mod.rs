//! `ExecutionCompiler` — `AthenaRequest` + Session snapshot → [`ExecutionModule`].
//!
//! Bootstrap lowering: atom terms, typed Boolean constants, `ControlPlan::Branch` /
//! `Sequence`, and effectful `SessionCommand::Define` via `WriteBinding`.
//! No bridge to the old stack VM / `KernelIR` path.

use athena_ir::{Atom, TermNode};
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

use crate::api::request::{AthenaRequest, ControlPlan, DefinitionEvaluationTiming, SessionCommand};
use crate::execution::ir::{
    BasicBlock, BlockEdge, BlockId, CapturedRoot, CapturedRootId, ConstantId, ConstantValue, EffectEdge, EffectKind,
    EffectToken, ExecutionModule, ExecutionValueType, ModuleFingerprint, Operation, OperationKind, ProviderCallDescriptor,
    ProviderCallId, Region, RegionId, SsaValueId, Terminator, verify_module,
};
use crate::runtime::session::Session;

/// Compiles one request into a verified [`ExecutionModule`].
#[derive(Debug, Default)]
pub struct ExecutionCompiler {}

#[derive(Default)]
struct ModuleBuilder {
    constants: Vec<ConstantValue>,
    captured_roots: Vec<CapturedRoot>,
    effect_edges: Vec<EffectEdge>,
    provider_calls: Vec<ProviderCallDescriptor>,
    next_ssa: u32,
    next_block: u32,
    next_effect: u32,
}

impl ModuleBuilder {
    fn ssa(&mut self) -> SsaValueId {
        let id = SsaValueId(self.next_ssa);
        self.next_ssa = self.next_ssa.saturating_add(1);
        id
    }

    fn block_id(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block = self.next_block.saturating_add(1);
        id
    }

    fn push_constant(&mut self, value: ConstantValue) -> ConstantId {
        let id = ConstantId(self.constants.len() as u32);
        self.constants.push(value);
        id
    }

    fn push_term_root(&mut self, term: TermId) -> CapturedRootId {
        let id = CapturedRootId(self.captured_roots.len() as u32);
        self.captured_roots.push(CapturedRoot::term(term));
        id
    }

    fn push_effect(&mut self, kind: EffectKind, precedes_from: Option<EffectToken>) -> EffectToken {
        let token = EffectToken(self.next_effect);
        self.next_effect = self.next_effect.saturating_add(1);
        self.effect_edges.push(match precedes_from {
            Some(prev) => EffectEdge::after(token, prev, kind),
            None => EffectEdge::entry(token, kind),
        });
        token
    }

    fn push_provider_call(&mut self, descriptor: ProviderCallDescriptor) -> ProviderCallId {
        let id = ProviderCallId(self.provider_calls.len() as u32);
        let mut descriptor = descriptor;
        descriptor.id = id;
        self.provider_calls.push(descriptor);
        id
    }

    fn finish(self, blocks: Vec<BasicBlock>, entry: BlockId) -> Result<ExecutionModule> {
        let region = Region {
            id: RegionId(0),
            entry,
            blocks,
            result_types: vec![ExecutionValueType::Term],
        };
        let mut module = ExecutionModule {
            inputs: Vec::new(),
            constants: self.constants,
            captured_roots: self.captured_roots,
            regions: vec![region],
            effect_edges: self.effect_edges,
            exits: Vec::new(),
            provider_calls: self.provider_calls,
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        verify_module(&module)?;
        Ok(module)
    }
}

impl ExecutionCompiler {
    /// Create a compiler instance.
    pub fn new() -> Self {
        Self {}
    }

    /// Lower a request against a Session snapshot into `ExecutionIR`.
    pub fn compile(&self, session: &Session, request: &AthenaRequest) -> Result<ExecutionModule> {
        let mut builder = ModuleBuilder::default();
        let entry = builder.block_id();
        let mut blocks = Vec::new();
        let value = self.lower_request(session, &mut builder, &mut blocks, entry, request)?;
        // Ensure entry block exists and returns when lowering produced a single block return.
        if blocks.iter().all(|b| b.id != entry) {
            blocks.insert(
                0,
                BasicBlock {
                    id: entry,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::return_value(value),
                },
            );
        }
        builder.finish(blocks, entry)
    }

    fn lower_request(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        request: &AthenaRequest,
    ) -> Result<SsaValueId> {
        match request {
            AthenaRequest::Term(term) => self.lower_term(session, builder, blocks, block_id, *term),
            AthenaRequest::Control(plan) => self.lower_control(session, builder, blocks, block_id, plan),
            AthenaRequest::Command(command) => self.lower_command(session, builder, blocks, block_id, command),
            AthenaRequest::Goal(_) => self.lower_goal_provider(builder, blocks, block_id),
        }
    }

    /// Lower a term request: `If` becomes SSA branch, other pure forms fill one block.
    fn lower_term(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        term: TermId,
    ) -> Result<SsaValueId> {
        if let Some(TermNode::Application { head, arguments }) = session.arena.get(term) {
            if session.operators.name(*head) == Some("If") {
                return match arguments.as_slice() {
                    [condition, then_branch] => {
                        let then_req = AthenaRequest::Term(*then_branch);
                        self.lower_branch(session, builder, blocks, block_id, *condition, &then_req, None)
                    }
                    [condition, then_branch, else_branch] => {
                        let then_req = AthenaRequest::Term(*then_branch);
                        let else_req = AthenaRequest::Term(*else_branch);
                        self.lower_branch(
                            session,
                            builder,
                            blocks,
                            block_id,
                            *condition,
                            &then_req,
                            Some(&else_req),
                        )
                    }
                    _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "if_arity_not_supported")),
                };
            }
        }
        self.lower_term_into_block(session, builder, blocks, block_id, term)
    }

    /// Domain goals lower to an explicit `CallProvider` + `PublishResult` edge.
    ///
    /// The `DomainRequest` payload is supplied at runtime by `execute_ir_request`
    /// (not stored in the module), so backends share the same IR shape.
    fn lower_goal_provider(
        &self,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
    ) -> Result<SsaValueId> {
        use athena_types::OperatorId;

        let call = builder.push_provider_call(ProviderCallDescriptor::new(
            ProviderCallId(0),
            OperatorId(0),
            ExecutionValueType::Unit,
        ));
        let effect_call_in = builder.push_effect(EffectKind::CallProvider, None);
        let effect_call_out = builder.push_effect(EffectKind::CallProvider, Some(effect_call_in));
        let payload = builder.ssa();
        let effect_pub_in = builder.push_effect(EffectKind::PublishResult, Some(effect_call_out));
        let effect_pub_out = builder.push_effect(EffectKind::PublishResult, Some(effect_pub_in));
        let published = builder.ssa();
        blocks.push(BasicBlock {
            id: block_id,
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    result: Some(payload),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::CallProvider {
                        call,
                        args: Vec::new(),
                    },
                    effect_in: Some(effect_call_in),
                    effect_out: Some(effect_call_out),
                },
                Operation {
                    result: Some(published),
                    result_type: ExecutionValueType::Unit,
                    kind: OperationKind::PublishResult { source: payload },
                    effect_in: Some(effect_pub_in),
                    effect_out: Some(effect_pub_out),
                },
            ],
            terminator: Terminator::return_value(published),
        });
        Ok(published)
    }

    fn lower_command(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        command: &SessionCommand,
    ) -> Result<SsaValueId> {
        match command {
            SessionCommand::Define {
                symbol,
                value,
                timing: DefinitionEvaluationTiming::Immediate | DefinitionEvaluationTiming::Deferred,
            } => {
                // Atom rhs only: Immediate and Deferred coincide for already-normalized atoms.
                self.require_atom(session, *value)?;
                let key = builder.ssa();
                let key_constant = builder.push_constant(ConstantValue::symbol(*symbol));
                let root = builder.push_term_root(*value);
                let rhs = builder.ssa();
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
                            result: Some(rhs),
                            result_type: ExecutionValueType::Term,
                            kind: OperationKind::LoadTerm { root },
                            effect_in: None,
                            effect_out: None,
                        },
                        Operation {
                            result: Some(unit),
                            result_type: ExecutionValueType::Unit,
                            kind: OperationKind::WriteBinding { key, value: rhs },
                            effect_in: Some(effect_in),
                            effect_out: Some(effect_out),
                        },
                    ],
                    terminator: Terminator::return_value(unit),
                });
                Ok(unit)
            }
            SessionCommand::ClearDefinition { symbol } => {
                let key = builder.ssa();
                let key_constant = builder.push_constant(ConstantValue::symbol(*symbol));
                let unit_const = builder.push_constant(ConstantValue::Unit);
                let unit_val = builder.ssa();
                let effect_in = builder.push_effect(EffectKind::WriteBinding, None);
                let effect_out = builder.push_effect(EffectKind::WriteBinding, Some(effect_in));
                let result = builder.ssa();
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
                            result: Some(unit_val),
                            result_type: ExecutionValueType::Unit,
                            kind: OperationKind::Constant { constant: unit_const },
                            effect_in: None,
                            effect_out: None,
                        },
                        Operation {
                            result: Some(result),
                            result_type: ExecutionValueType::Unit,
                            // Unit rhs means clear binding (not store Unit as Own).
                            kind: OperationKind::WriteBinding { key, value: unit_val },
                            effect_in: Some(effect_in),
                            effect_out: Some(effect_out),
                        },
                    ],
                    terminator: Terminator::return_value(result),
                });
                Ok(result)
            }
        }
    }

    fn lower_control(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        plan: &ControlPlan,
    ) -> Result<SsaValueId> {
        match plan {
            ControlPlan::Sequence { steps } => self.lower_sequence(session, builder, blocks, block_id, steps),
            ControlPlan::Branch {
                condition,
                then_branch,
                else_branch,
            } => self.lower_branch(session, builder, blocks, block_id, *condition, then_branch, else_branch.as_deref()),
            ControlPlan::LocalScope { body } | ControlPlan::LexicalScope { body } | ControlPlan::DynamicScope { body } => {
                self.lower_scope(session, builder, blocks, block_id, body)
            }
            ControlPlan::Cond { arms, otherwise } => {
                self.lower_cond(session, builder, blocks, block_id, arms, otherwise.as_deref())
            }
            ControlPlan::Recover { body, handler } => {
                self.lower_recover(session, builder, blocks, block_id, body, handler)
            }
            ControlPlan::LoopWhile { condition, body } => {
                self.lower_loop_while(session, builder, blocks, block_id, *condition, body)
            }
            ControlPlan::CountedLoop {
                variable,
                iterator,
                body,
            } => self.lower_counted_loop(session, builder, blocks, block_id, *variable, *iterator, body),
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "control_plan_not_lowered")),
        }
    }

    fn lower_counted_loop(
        &self,
        session: &Session,
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
        self.require_atom(session, *body_term)?;

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
                timing: DefinitionEvaluationTiming::Immediate,
            }));
            steps.push(AthenaRequest::Term(*body_term));
        }
        let budget_in = builder.push_effect(EffectKind::BudgetCheck, None);
        let _budget_out = builder.push_effect(EffectKind::BudgetCheck, Some(budget_in));
        self.lower_sequence(session, builder, blocks, entry, &steps)
    }

    fn require_symbol_atom(&self, session: &Session, term: TermId) -> Result<athena_types::SymbolId> {
        match session.arena.get(term) {
            Some(TermNode::Atom(Atom::Symbol(symbol))) => Ok(*symbol),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "counted_loop_variable_not_symbol")),
            None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("component", "ExecutionCompiler")
                .detail("reason", "missing_term")),
        }
    }

    fn require_atom_list(&self, session: &Session, term: TermId) -> Result<Vec<TermId>> {
        match session.arena.get(term) {
            Some(TermNode::List(items)) => {
                for item in items {
                    self.require_atom(session, *item)?;
                }
                Ok(items.clone())
            }
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "counted_loop_iterator_not_atom_list")),
            None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("component", "ExecutionCompiler")
                .detail("reason", "missing_term")),
        }
    }

    fn lower_loop_while(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        condition: TermId,
        body: &AthenaRequest,
    ) -> Result<SsaValueId> {
        let cond_bool = self.require_boolean_atom(session, condition)?;
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
                then_edge: BlockEdge {
                    target: header,
                    arguments: vec![init],
                },
                else_edge: BlockEdge {
                    target: header,
                    arguments: vec![init],
                },
            },
        });

        let loop_cond = builder.ssa();
        let loop_const = builder.push_constant(ConstantValue::boolean(cond_bool));
        blocks.push(BasicBlock {
            id: header,
            parameters: vec![crate::execution::ir::BlockParameter {
                value: acc_param,
                ty: ExecutionValueType::Term,
            }],
            operations: vec![Operation {
                result: Some(loop_cond),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: loop_const },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::Branch {
                condition: loop_cond,
                then_edge: BlockEdge::jump(body_block),
                else_edge: BlockEdge {
                    target: exit,
                    arguments: vec![acc_param],
                },
            },
        });

        let body_value = self.lower_request(session, builder, blocks, body_block, body)?;
        // Body returns continue at header with the new accumulator.
        self.rewrite_returns_to_join(builder, blocks, header, body_value)?;

        blocks.push(BasicBlock {
            id: exit,
            parameters: vec![crate::execution::ir::BlockParameter {
                value: exit_param,
                ty: ExecutionValueType::Term,
            }],
            operations: Vec::new(),
            terminator: Terminator::return_value(exit_param),
        });
        Ok(exit_param)
    }

    fn lower_recover(
        &self,
        session: &Session,
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
            parameters: vec![crate::execution::ir::BlockParameter {
                value: result_param,
                ty: ExecutionValueType::Term,
            }],
            operations: Vec::new(),
            terminator: Terminator::return_value(result_param),
        });
        Ok(result_param)
    }

    fn rewrite_rejects_to_handler(
        &self,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        handler: BlockId,
    ) -> Result<()> {
        let reject_ids: Vec<BlockId> = blocks
            .iter()
            .filter(|b| matches!(b.terminator, Terminator::Reject { .. }))
            .map(|b| b.id)
            .collect();
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
            block.terminator = Terminator::Branch {
                condition: cond,
                then_edge: BlockEdge::jump(handler),
                else_edge: BlockEdge::jump(handler),
            };
        }
        Ok(())
    }

    fn lower_cond(
        &self,
        session: &Session,
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
            let cond_bool = self.require_boolean_atom(session, *condition)?;
            let cond_value = builder.ssa();
            let cond_constant = builder.push_constant(ConstantValue::boolean(cond_bool));
            let arm_block = builder.block_id();
            let else_target = if index + 1 < arms.len() {
                test_blocks[index + 1]
            } else {
                otherwise_block
            };
            blocks.push(BasicBlock {
                id: test_blocks[index],
                parameters: Vec::new(),
                operations: vec![Operation {
                    result: Some(cond_value),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: cond_constant },
                    effect_in: None,
                    effect_out: None,
                }],
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
            parameters: vec![crate::execution::ir::BlockParameter {
                value: result_param,
                ty: ExecutionValueType::Term,
            }],
            operations: Vec::new(),
            terminator: Terminator::return_value(result_param),
        });
        Ok(result_param)
    }

    /// Rewrite current `Return` terminators into jumps to `join` with a block argument.
    fn rewrite_returns_to_join(
        &self,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        join: BlockId,
        fallback: SsaValueId,
    ) -> Result<()> {
        let return_block_ids: Vec<BlockId> = blocks
            .iter()
            .filter(|b| matches!(b.terminator, Terminator::Return { .. }))
            .map(|b| b.id)
            .collect();
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
                then_edge: BlockEdge {
                    target: join,
                    arguments: vec![forwarded],
                },
                else_edge: BlockEdge {
                    target: join,
                    arguments: vec![forwarded],
                },
            };
        }
        Ok(())
    }

    fn lower_scope(
        &self,
        session: &Session,
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
        let return_block_ids: Vec<BlockId> = blocks
            .iter()
            .filter(|b| matches!(b.terminator, Terminator::Return { .. }))
            .map(|b| b.id)
            .collect();
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
                then_edge: BlockEdge {
                    target: exit_block,
                    arguments: vec![forwarded],
                },
                else_edge: BlockEdge {
                    target: exit_block,
                    arguments: vec![forwarded],
                },
            };
        }

        let result_param = builder.ssa();
        let exit_in = builder.push_effect(EffectKind::ExitScope, Some(enter_out));
        let exit_out = builder.push_effect(EffectKind::ExitScope, Some(exit_in));
        blocks.push(BasicBlock {
            id: exit_block,
            parameters: vec![crate::execution::ir::BlockParameter {
                value: result_param,
                ty: ExecutionValueType::Term,
            }],
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

    fn lower_sequence(
        &self,
        session: &Session,
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
                let cond = builder.ssa();
                let c = builder.push_constant(ConstantValue::boolean(true));
                let block = blocks
                    .iter_mut()
                    .find(|b| b.id == block_id)
                    .ok_or_else(|| {
                        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                            .detail("component", "ExecutionCompiler")
                            .detail("status", "sequence_block_missing")
                    })?;
                block.operations.push(Operation {
                    result: Some(cond),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant: c },
                    effect_in: None,
                    effect_out: None,
                });
                // Chain steps with explicit jumps (no demand queue).
                block.terminator = Terminator::Branch {
                    condition: cond,
                    then_edge: BlockEdge::jump(next),
                    else_edge: BlockEdge::jump(next),
                };
            }
        }
        Ok(last_value)
    }

    fn lower_branch(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        entry: BlockId,
        condition: TermId,
        then_branch: &AthenaRequest,
        else_branch: Option<&AthenaRequest>,
    ) -> Result<SsaValueId> {
        let cond_bool = self.require_boolean_atom(session, condition)?;
        let cond_value = builder.ssa();
        let cond_constant = builder.push_constant(ConstantValue::boolean(cond_bool));
        let then_block = builder.block_id();
        let else_block = builder.block_id();

        blocks.push(BasicBlock {
            id: entry,
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(cond_value),
                result_type: ExecutionValueType::Boolean,
                kind: OperationKind::Constant { constant: cond_constant },
                effect_in: None,
                effect_out: None,
            }],
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

    fn lower_unit_else(
        &self,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        else_block: BlockId,
        then_value: SsaValueId,
    ) -> Result<SsaValueId> {
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

    fn lower_term_into_block(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        blocks: &mut Vec<BasicBlock>,
        block_id: BlockId,
        term: TermId,
    ) -> Result<SsaValueId> {
        let mut operations = Vec::new();
        let value = self.lower_pure_expr(session, builder, &mut operations, term)?;
        blocks.push(BasicBlock {
            id: block_id,
            parameters: Vec::new(),
            operations,
            terminator: Terminator::return_value(value),
        });
        Ok(value)
    }

    /// Lower pure atom / Boolean semantic applications into SSA ops (no Session effects).
    fn lower_pure_expr(
        &self,
        session: &Session,
        builder: &mut ModuleBuilder,
        operations: &mut Vec<Operation>,
        term: TermId,
    ) -> Result<SsaValueId> {
        match session.arena.get(term) {
            Some(TermNode::Atom(Atom::Boolean(value))) => {
                let ssa = builder.ssa();
                let constant = builder.push_constant(ConstantValue::boolean(*value));
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::Constant { constant },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            Some(TermNode::Atom(Atom::Symbol(symbol))) => {
                let key = builder.ssa();
                let key_constant = builder.push_constant(ConstantValue::symbol(*symbol));
                let effect_in = builder.push_effect(EffectKind::ReadBinding, None);
                let effect_out = builder.push_effect(EffectKind::ReadBinding, Some(effect_in));
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(key),
                    result_type: ExecutionValueType::Symbol,
                    kind: OperationKind::Constant { constant: key_constant },
                    effect_in: None,
                    effect_out: None,
                });
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::ReadBinding { key },
                    effect_in: Some(effect_in),
                    effect_out: Some(effect_out),
                });
                Ok(ssa)
            }
            Some(TermNode::Atom(_)) => {
                let root = builder.push_term_root(term);
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::LoadTerm { root },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            Some(TermNode::Application { head, arguments }) => {
                let name = session.operators.name(*head).ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "unknown_operator")
                })?;
                let result_type = match name {
                    "Not" | "And" | "Or" | "TrueQ" | "SameQ" | "Equal" | "Unequal"
                    | "Less" | "Greater" | "LessEqual" | "GreaterEqual" => ExecutionValueType::Boolean,
                    "Plus" | "Times" | "Subtract" | "Divide" | "Power" | "Abs" | "Length"
                    | "First" | "Rest" | "Join" | "Factorial" | "Part" | "Span" | "Range" | "Sqrt"
                    | "Apply" | "Size" | "Map" | "Zeros" | "Ones" | "Eye"
                    | "ReplaceAll" | "Rule" | "RuleDeferred" | "Simplify"
                    | "Sin" | "Cos" | "Tan" | "Exp" | "Log" => ExecutionValueType::Term,
                    _ => {
                        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                            .detail("component", "ExecutionCompiler")
                            .detail("status", "compound_term_not_lowered")
                            .detail("operator", name));
                    }
                };
                let mut args = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    args.push(self.lower_pure_expr(session, builder, operations, *arg)?);
                }
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(ssa),
                    result_type,
                    kind: OperationKind::ApplySemanticOperator {
                        operator: *head,
                        args,
                    },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            Some(TermNode::List(items)) => {
                let mut elements = Vec::with_capacity(items.len());
                for item in items {
                    elements.push(self.lower_pure_expr(session, builder, operations, *item)?);
                }
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Term,
                    kind: OperationKind::MakeList { elements },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("component", "ExecutionCompiler")
                .detail("reason", "missing_term")),
        }
    }

    fn require_atom(&self, session: &Session, term: TermId) -> Result<()> {
        match session.arena.get(term) {
            Some(TermNode::Atom(_)) => Ok(()),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "compound_term_not_lowered")),
            None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("component", "ExecutionCompiler")
                .detail("reason", "missing_term")),
        }
    }

    fn require_boolean_atom(&self, session: &Session, term: TermId) -> Result<bool> {
        match session.arena.get(term) {
            Some(TermNode::Atom(Atom::Boolean(value))) => Ok(*value),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "branch_condition_not_boolean_atom")),
            None => Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("component", "ExecutionCompiler")
                .detail("reason", "missing_term")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::reference::ReferenceExecutor;
    use crate::runtime::session::Session;
    use athena_types::ComputationStatus;

    #[test]
    fn compile_atom_term_module() {
        let mut session = Session::new();
        let term = session.builder().int(3, Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("atom");
        assert_eq!(module.captured_roots, vec![CapturedRoot::term(term)]);
        assert_eq!(module.regions.len(), 1);
    }

    #[test]
    fn compile_and_execute_plus_integers() {
        let mut session = Session::new();
        let a = session.builder().int(2, Default::default());
        let b = session.builder().int(3, Default::default());
        let plus = session.operators.intern("Plus");
        let term = session.builder().application(plus, vec![a, b], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("plus");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5) => {}
            other => panic!("expected Plus[2,3] == 5, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_less_chain() {
        let mut session = Session::new();
        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(2, Default::default());
        let c = session.builder().int(4, Default::default());
        let less = session.operators.intern("Less");
        let term = session.builder().application(less, vec![a, b, c], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("less");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Boolean(true))) => {}
            other => panic!("expected Less[1,2,4] == True, got {other:?}"),
        }

        let x = session.builder().int(3, Default::default());
        let y = session.builder().int(1, Default::default());
        let bad = session.builder().application(less, vec![x, y], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(bad))
            .expect("less2");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Boolean(false))) => {}
            other => panic!("expected Less[3,1] == False, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_list_with_plus() {
        let mut session = Session::new();
        let a = session.builder().int(2, Default::default());
        let b = session.builder().int(3, Default::default());
        let plus = session.operators.intern("Plus");
        let sum = session.builder().application(plus, vec![a, b], Default::default());
        let c = session.builder().int(9, Default::default());
        let list = session.builder().list(vec![sum, c], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(list))
            .expect("list");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(items)) if items.len() == 2 => {
                match session.arena.get(items[0]) {
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5) => {}
                    other => panic!("expected first element 5, got {other:?}"),
                }
                assert_eq!(items[1], c);
            }
            other => panic!("expected List[5,9], got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_abs_and_length() {
        let mut session = Session::new();
        let n = session.builder().int(-7, Default::default());
        let abs = session.operators.intern("Abs");
        let abs_term = session.builder().application(abs, vec![n], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(abs_term))
            .expect("abs");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(v))) if v.as_exact_integer() == Some(7) => {}
            other => panic!("expected Abs[-7] == 7, got {other:?}"),
        }

        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(2, Default::default());
        let list = session.builder().list(vec![a, b], Default::default());
        let length = session.operators.intern("Length");
        let length_term = session.builder().application(length, vec![list], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(length_term))
            .expect("length");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(v))) if v.as_exact_integer() == Some(2) => {}
            other => panic!("expected Length[List[1,2]] == 2, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_first_rest_join() {
        let mut session = Session::new();
        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(2, Default::default());
        let c = session.builder().int(3, Default::default());
        let left = session.builder().list(vec![a, b], Default::default());
        let right = session.builder().list(vec![c], Default::default());
        let join = session.operators.intern("Join");
        let joined = session.builder().application(join, vec![left, right], Default::default());
        let first = session.operators.intern("First");
        let rest = session.operators.intern("Rest");
        let first_term = session.builder().application(first, vec![joined], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(first_term))
            .expect("first");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        assert_eq!(
            session.results.get(result_id).expect("result").symbolic_term,
            Some(a)
        );

        let list = session.builder().list(vec![a, b, c], Default::default());
        let rest_term = session.builder().application(rest, vec![list], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(rest_term))
            .expect("rest");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(items)) if items.as_slice() == [b, c] => {}
            other => panic!("expected Rest == List[2,3], got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_factorial() {
        let mut session = Session::new();
        let n = session.builder().int(5, Default::default());
        let fact = session.operators.intern("Factorial");
        let term = session.builder().application(fact, vec![n], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("factorial");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(v))) if v.as_exact_integer() == Some(120) => {}
            other => panic!("expected Factorial[5] == 120, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_part_list() {
        let mut session = Session::new();
        let a = session.builder().int(10, Default::default());
        let b = session.builder().int(20, Default::default());
        let c = session.builder().int(30, Default::default());
        let list = session.builder().list(vec![a, b, c], Default::default());
        let idx = session.builder().int(2, Default::default());
        let part = session.operators.intern("Part");
        let term = session.builder().application(part, vec![list, idx], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("part");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        assert_eq!(
            session.results.get(result_id).expect("result").symbolic_term,
            Some(b)
        );

        let idx_neg = session.builder().int(-1, Default::default());
        let term = session.builder().application(part, vec![list, idx_neg], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("part_neg");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        assert_eq!(
            session.results.get(result_id).expect("result").symbolic_term,
            Some(c)
        );
    }

    #[test]
    fn compile_and_execute_span() {
        let mut session = Session::new();
        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(3, Default::default());
        let span = session.operators.intern("Span");
        let term = session.builder().application(span, vec![a, b], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("span");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(items)) if items.len() == 3 => {
                for (i, expected) in [1i64, 2, 3].into_iter().enumerate() {
                    match session.arena.get(items[i]) {
                        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(expected) => {}
                        other => panic!("expected Span element {expected}, got {other:?}"),
                    }
                }
            }
            other => panic!("expected Span[1,3] == List[1,2,3], got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_range_and_sqrt() {
        let mut session = Session::new();
        let n = session.builder().int(3, Default::default());
        let range = session.operators.intern("Range");
        let term = session.builder().application(range, vec![n], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("range");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(items)) if items.len() == 3 => {}
            other => panic!("expected Range[3] length 3, got {other:?}"),
        }

        let four = session.builder().int(4, Default::default());
        let sqrt = session.operators.intern("Sqrt");
        let term = session.builder().application(sqrt, vec![four], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("sqrt");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(v))) if v.as_exact_integer() == Some(2) => {}
            other => panic!("expected Sqrt[4] == 2, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_apply_and_size() {
        let mut session = Session::new();
        let one = session.builder().int(1, Default::default());
        let two = session.builder().int(2, Default::default());
        let list = session.builder().list(vec![one, two], Default::default());
        let plus = session.builder().symbol("Plus", Default::default());
        let apply = session.operators.intern("Apply");
        let term = session.builder().application(apply, vec![plus, list], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("apply");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3) => {}
            other => panic!("expected Apply[Plus, List[1,2]] == 3, got {other:?}"),
        }

        let row = session.builder().list(vec![one, two], Default::default());
        let matrix = session.builder().list(vec![row, row], Default::default());
        let size = session.operators.intern("Size");
        let term = session.builder().application(size, vec![matrix], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("size");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(items)) if items.len() == 2 => {
                for (i, expected) in [2i64, 2].into_iter().enumerate() {
                    match session.arena.get(items[i]) {
                        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(expected) => {}
                        other => panic!("expected Size dim {expected}, got {other:?}"),
                    }
                }
            }
            other => panic!("expected Size == List[2,2], got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_map_symbol() {
        let mut session = Session::new();
        let a = session.builder().int(-1, Default::default());
        let b = session.builder().int(4, Default::default());
        let list = session.builder().list(vec![a, b], Default::default());
        let abs = session.builder().symbol("Abs", Default::default());
        let map = session.operators.intern("Map");
        let term = session.builder().application(map, vec![abs, list], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("map");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(items)) if items.len() == 2 => {
                match session.arena.get(items[0]) {
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1) => {}
                    other => panic!("expected Abs[-1]==1, got {other:?}"),
                }
                match session.arena.get(items[1]) {
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(4) => {}
                    other => panic!("expected Abs[4]==4, got {other:?}"),
                }
            }
            other => panic!("expected Map[Abs, List[-1,4]] == List[1,4], got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_zeros_eye() {
        let mut session = Session::new();
        let two = session.builder().int(2, Default::default());
        let zeros = session.operators.intern("Zeros");
        let term = session.builder().application(zeros, vec![two], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("zeros");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(rows)) if rows.len() == 2 => {
                for row in rows {
                    match session.arena.get(*row) {
                        Some(TermNode::List(cells)) if cells.len() == 2 => {
                            for cell in cells {
                                match session.arena.get(*cell) {
                                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0) => {}
                                    other => panic!("expected 0, got {other:?}"),
                                }
                            }
                        }
                        other => panic!("expected row List, got {other:?}"),
                    }
                }
            }
            other => panic!("expected Zeros[2] 2x2, got {other:?}"),
        }

        let eye = session.operators.intern("Eye");
        let term = session.builder().application(eye, vec![two], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("eye");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(rows)) if rows.len() == 2 => {
                let expected = [[1i64, 0], [0, 1]];
                for (i, row) in rows.iter().enumerate() {
                    match session.arena.get(*row) {
                        Some(TermNode::List(cells)) if cells.len() == 2 => {
                            for (j, cell) in cells.iter().enumerate() {
                                match session.arena.get(*cell) {
                                    Some(TermNode::Atom(Atom::Number(n)))
                                        if n.as_exact_integer() == Some(expected[i][j]) => {}
                                    other => panic!("expected Eye[{i},{j}]={}, got {other:?}", expected[i][j]),
                                }
                            }
                        }
                        other => panic!("expected Eye row, got {other:?}"),
                    }
                }
            }
            other => panic!("expected Eye[2], got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_replace_all() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", Default::default());
        let one = session.builder().int(1, Default::default());
        let two = session.builder().int(2, Default::default());
        let plus = session.operators.intern("Plus");
        let expr = session.builder().application(plus, vec![x, one], Default::default());
        let rule_op = session.operators.intern("Rule");
        let rule = session.builder().application(rule_op, vec![x, two], Default::default());
        let replace = session.operators.intern("ReplaceAll");
        let term = session.builder().application(replace, vec![expr, rule], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("replace");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3) => {}
            other => panic!("expected ReplaceAll[Plus[x,1], x->2] == 3, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_simplify_pythagorean() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", Default::default());
        let sin = session.operators.intern("Sin");
        let cos = session.operators.intern("Cos");
        let power = session.operators.intern("Power");
        let plus = session.operators.intern("Plus");
        let two = session.builder().int(2, Default::default());
        let sin_x = session.builder().application(sin, vec![x], Default::default());
        let cos_x = session.builder().application(cos, vec![x], Default::default());
        let sin2 = session.builder().application(power, vec![sin_x, two], Default::default());
        let cos2 = session.builder().application(power, vec![cos_x, two], Default::default());
        let sum = session.builder().application(plus, vec![sin2, cos2], Default::default());
        let simplify = session.operators.intern("Simplify");
        let term = session.builder().application(simplify, vec![sum], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("simplify");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1) => {}
            other => panic!("expected Simplify[Sin[x]^2+Cos[x]^2] == 1, got {other:?}"),
        }
    }

    #[test]
    fn compile_application_rejected() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", Default::default());
        let head = session.operators.intern("Foo");
        let term = session.builder().application(head, vec![x], Default::default());
        let err = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect_err("compound");
        assert_eq!(err.code, DiagnosticCode::UnsupportedOperation);
    }

    #[test]
    fn compile_and_execute_boolean_branch() {
        let mut session = Session::new();
        let cond = session.builder().boolean(true, Default::default());
        let then_term = session.builder().int(1, Default::default());
        let else_term = session.builder().int(0, Default::default());
        let request = AthenaRequest::Control(ControlPlan::Branch {
            condition: cond,
            then_branch: Box::new(AthenaRequest::Term(then_term)),
            else_branch: Some(Box::new(AthenaRequest::Term(else_term))),
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("branch");
        assert_eq!(module.regions[0].blocks.len(), 3);
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(then_term));
        assert_eq!(loaded.status, ComputationStatus::Exact);
    }

    #[test]
    fn compile_and_execute_define_write_binding() {
        use crate::api::request::{DefinitionEvaluationTiming, SessionCommand};

        let mut session = Session::new();
        let sym_term = session.builder().symbol("x", Default::default());
        let symbol = match session.arena.get(sym_term) {
            Some(TermNode::Atom(Atom::Symbol(id))) => *id,
            other => panic!("expected symbol atom, got {other:?}"),
        };
        let value = session.builder().int(42, Default::default());
        let request = AthenaRequest::Command(SessionCommand::Define {
            symbol,
            value,
            timing: DefinitionEvaluationTiming::Immediate,
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("define");
        assert!(!module.effect_edges.is_empty());
        ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.defs.own(symbol), Some(value));
    }

    #[test]
    fn compile_and_execute_define_then_read_binding() {
        use crate::api::request::{DefinitionEvaluationTiming, SessionCommand};

        let mut session = Session::new();
        let sym_term = session.builder().symbol("y", Default::default());
        let symbol = match session.arena.get(sym_term) {
            Some(TermNode::Atom(Atom::Symbol(id))) => *id,
            other => panic!("expected symbol atom, got {other:?}"),
        };
        let value = session.builder().int(7, Default::default());
        let define = AthenaRequest::Command(SessionCommand::Define {
            symbol,
            value,
            timing: DefinitionEvaluationTiming::Immediate,
        });
        let define_module = ExecutionCompiler::new().compile(&session, &define).expect("define");
        ReferenceExecutor::new().execute(&mut session, &define_module, None).expect("define exec");

        let read = AthenaRequest::Term(sym_term);
        let read_module = ExecutionCompiler::new().compile(&session, &read).expect("read");
        let result_id = ReferenceExecutor::new().execute(&mut session, &read_module, None).expect("read exec");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(value));
    }

    #[test]
    fn compile_and_execute_sequence_define_read_clear() {
        use crate::api::request::{DefinitionEvaluationTiming, SessionCommand};

        let mut session = Session::new();
        let sym_term = session.builder().symbol("z", Default::default());
        let symbol = match session.arena.get(sym_term) {
            Some(TermNode::Atom(Atom::Symbol(id))) => *id,
            other => panic!("expected symbol atom, got {other:?}"),
        };
        let value = session.builder().int(5, Default::default());
        let request = AthenaRequest::Control(ControlPlan::Sequence {
            steps: vec![
                AthenaRequest::Command(SessionCommand::Define {
                    symbol,
                    value,
                    timing: DefinitionEvaluationTiming::Immediate,
                }),
                AthenaRequest::Term(sym_term),
                AthenaRequest::Command(SessionCommand::ClearDefinition { symbol }),
            ],
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("sequence");
        assert_eq!(module.regions[0].blocks.len(), 3);
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        // Last step clears; result is Unit → Null term.
        match session.arena.get(loaded.symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Null)) => {}
            other => panic!("expected Null after clear, got {other:?}"),
        }
        assert!(session.defs.own(symbol).is_none());
    }

    #[test]
    fn compile_and_execute_counted_loop_unroll() {
        let mut session = Session::new();
        let var = session.builder().symbol("i", Default::default());
        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(2, Default::default());
        let c = session.builder().int(3, Default::default());
        let iter = session.builder().list(vec![a, b, c], Default::default());
        let request = AthenaRequest::Control(ControlPlan::CountedLoop {
            variable: var,
            iterator: iter,
            body: Box::new(AthenaRequest::Term(var)),
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("counted");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(c));
        let symbol = match session.arena.get(var) {
            Some(TermNode::Atom(Atom::Symbol(id))) => *id,
            other => panic!("expected symbol, got {other:?}"),
        };
        assert_eq!(session.defs.own(symbol), Some(c));
    }

    #[test]
    fn compile_and_execute_loop_while_false() {
        let mut session = Session::new();
        let cond = session.builder().boolean(false, Default::default());
        let body = session.builder().int(1, Default::default());
        let request = AthenaRequest::Control(ControlPlan::LoopWhile {
            condition: cond,
            body: Box::new(AthenaRequest::Term(body)),
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("loop");
        assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::BudgetCheck)));
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        match session.arena.get(loaded.symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Null)) => {}
            other => panic!("expected Unit/Null after zero-trip loop, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_goal_call_provider_dispatches_domain() {
        use crate::api::request::DomainGoal;
        use crate::domains::dispatch::DomainRequest;
        use crate::domains::number_theory::NumberTheoryRequest;
        use crate::execution::execute_ir_request;
        use crate::runtime::values::RuntimeValue;
        use athena_numeric::Integer;

        let mut session = Session::new();
        let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
            a: Integer::from_i64(12),
            b: Integer::from_i64(8),
        })));
        let module = ExecutionCompiler::new().compile(&session, &request).expect("goal");
        assert_eq!(module.provider_calls.len(), 1);
        assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::CallProvider)));
        assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::PublishResult)));
        let result_id = execute_ir_request(&mut session, request).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.coverage, crate::runtime::results::CoverageStatus::Full);
        let value_id = loaded.value.expect("value");
        match session.values.get(value_id).expect("runtime") {
            RuntimeValue::Domain(crate::domains::dispatch::DomainResult::NumberTheory(
                crate::domains::number_theory::NumberTheoryResult::Exact {
                    value: crate::domains::number_theory::NumberTheoryValue::Integer(n),
                },
            )) => assert_eq!(n, &Integer::from_i64(4)),
            other => panic!("expected NumberTheory Exact Integer gcd, got {other:?}"),
        }
    }

    #[test]
    fn call_provider_without_domain_stays_unsupported() {
        use crate::api::request::DomainGoal;
        use crate::domains::dispatch::DomainRequest;
        use crate::domains::number_theory::NumberTheoryRequest;
        use athena_numeric::Integer;

        let mut session = Session::new();
        let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
            a: Integer::from_i64(12),
            b: Integer::from_i64(8),
        })));
        let module = ExecutionCompiler::new().compile(&session, &request).expect("goal");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.coverage, crate::runtime::results::CoverageStatus::Unsupported);
        assert!(!loaded.diagnostics.is_empty());
    }

    #[test]
    fn compile_and_execute_recover_success_body() {
        let mut session = Session::new();
        let body = session.builder().int(8, Default::default());
        let handler = session.builder().int(9, Default::default());
        let request = AthenaRequest::Control(ControlPlan::Recover {
            body: Box::new(AthenaRequest::Term(body)),
            handler: Box::new(AthenaRequest::Term(handler)),
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("recover");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(body));
    }

    #[test]
    fn compile_and_execute_cond_second_arm() {
        let mut session = Session::new();
        let c0 = session.builder().boolean(false, Default::default());
        let c1 = session.builder().boolean(true, Default::default());
        let a0 = session.builder().int(10, Default::default());
        let a1 = session.builder().int(20, Default::default());
        let otherwise = session.builder().int(30, Default::default());
        let request = AthenaRequest::Control(ControlPlan::Cond {
            arms: vec![
                (c0, Box::new(AthenaRequest::Term(a0))),
                (c1, Box::new(AthenaRequest::Term(a1))),
            ],
            otherwise: Some(Box::new(AthenaRequest::Term(otherwise))),
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("cond");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(a1));
    }

    #[test]
    fn compile_and_execute_local_scope_body() {
        let mut session = Session::new();
        let term = session.builder().int(11, Default::default());
        let request = AthenaRequest::Control(ControlPlan::LocalScope {
            body: Box::new(AthenaRequest::Term(term)),
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("scope");
        assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::EnterScope)));
        assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::ExitScope)));
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(term));
    }

    #[test]
    fn compile_and_execute_local_scope_shadows_session() {
        use crate::api::request::{DefinitionEvaluationTiming, SessionCommand};

        let mut session = Session::new();
        let sym_term = session.builder().symbol("s", Default::default());
        let symbol = match session.arena.get(sym_term) {
            Some(TermNode::Atom(Atom::Symbol(id))) => *id,
            other => panic!("expected symbol, got {other:?}"),
        };
        let global = session.builder().int(1, Default::default());
        let local = session.builder().int(2, Default::default());
        session.defs.define_own(symbol, global);

        let request = AthenaRequest::Control(ControlPlan::LocalScope {
            body: Box::new(AthenaRequest::Control(ControlPlan::Sequence {
                steps: vec![
                    AthenaRequest::Command(SessionCommand::Define {
                        symbol,
                        value: local,
                        timing: DefinitionEvaluationTiming::Immediate,
                    }),
                    AthenaRequest::Term(sym_term),
                ],
            })),
        });
        let module = ExecutionCompiler::new().compile(&session, &request).expect("scope");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(local));
        // Session Own unchanged after local scope exits.
        assert_eq!(session.defs.own(symbol), Some(global));
    }

    #[test]
    fn compile_and_execute_boolean_not_and() {
        let mut session = Session::new();
        let t = session.builder().boolean(true, Default::default());
        let f = session.builder().boolean(false, Default::default());
        let and = session.operators.intern("And");
        let not = session.operators.intern("Not");
        let and_term = session.builder().application(and, vec![t, f], Default::default());
        let term = session.builder().application(not, vec![and_term], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("bool ops");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::Atom(Atom::Boolean(true))) => {}
            other => panic!("expected Not[And[True,False]] == True, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_term_if() {
        let mut session = Session::new();
        let cond = session.builder().boolean(true, Default::default());
        let then_term = session.builder().int(11, Default::default());
        let else_term = session.builder().int(22, Default::default());
        let if_op = session.operators.intern("If");
        let term = session
            .builder()
            .application(if_op, vec![cond, then_term, else_term], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("if");
        assert_eq!(module.regions[0].blocks.len(), 3);
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(then_term));
    }

    #[test]
    fn compile_and_execute_sameq_and_trueq() {
        let mut session = Session::new();
        let t = session.builder().boolean(true, Default::default());
        let f = session.builder().boolean(false, Default::default());
        let same = session.operators.intern("SameQ");
        let true_q = session.operators.intern("TrueQ");
        let same_term = session.builder().application(same, vec![t, f], Default::default());
        let term = session.builder().application(true_q, vec![same_term], Default::default());
        // TrueQ[SameQ[True,False]] == TrueQ[False] == False
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("sameq");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        match session.arena.get(loaded.symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Boolean(false))) => {}
            other => panic!("expected False, got {other:?}"),
        }

        let a = session.builder().int(3, Default::default());
        let b = session.builder().int(3, Default::default());
        let eq = session.operators.intern("Equal");
        let eq_term = session.builder().application(eq, vec![a, b], Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(eq_term))
            .expect("equal");
        let result_id = ReferenceExecutor::new()
            .execute(&mut session, &module, None)
            .expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        match session.arena.get(loaded.symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Boolean(true))) => {}
            other => panic!("expected Equal[3,3] == True, got {other:?}"),
        }
    }
}
