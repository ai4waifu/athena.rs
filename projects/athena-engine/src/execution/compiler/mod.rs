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
            AthenaRequest::Term(term) => self.lower_term_into_block(session, builder, blocks, block_id, *term),
            AthenaRequest::Control(plan) => self.lower_control(session, builder, blocks, block_id, plan),
            AthenaRequest::Command(command) => self.lower_command(session, builder, blocks, block_id, command),
            AthenaRequest::Goal(_) => self.lower_goal_provider(builder, blocks, block_id),
        }
    }

    /// Bootstrap domain goals as an explicit `CallProvider` + `PublishResult` edge.
    ///
    /// Full `DomainRequest` payload binding lands with provider ABI wiring; this
    /// slice only freezes the IR shape and unsupported replay status.
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
                if !matches!(name, "Not" | "And" | "Or") {
                    return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionCompiler")
                        .detail("status", "compound_term_not_lowered")
                        .detail("operator", name));
                }
                let mut args = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    args.push(self.lower_pure_expr(session, builder, operations, *arg)?);
                }
                let ssa = builder.ssa();
                operations.push(Operation {
                    result: Some(ssa),
                    result_type: ExecutionValueType::Boolean,
                    kind: OperationKind::ApplySemanticOperator {
                        operator: *head,
                        args,
                    },
                    effect_in: None,
                    effect_out: None,
                });
                Ok(ssa)
            }
            Some(TermNode::List(_)) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "list_term_not_lowered")),
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
    fn compile_application_rejected() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", Default::default());
        let plus = session.operators.intern("Plus");
        let term = session.builder().application(plus, vec![x, x], Default::default());
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
        ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
        ReferenceExecutor::new().execute(&mut session, &define_module).expect("define exec");

        let read = AthenaRequest::Term(sym_term);
        let read_module = ExecutionCompiler::new().compile(&session, &read).expect("read");
        let result_id = ReferenceExecutor::new().execute(&mut session, &read_module).expect("read exec");
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        match session.arena.get(loaded.symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Null)) => {}
            other => panic!("expected Unit/Null after zero-trip loop, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_goal_call_provider_unsupported() {
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
        assert_eq!(module.provider_calls.len(), 1);
        assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::CallProvider)));
        assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::PublishResult)));
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::Atom(Atom::Boolean(true))) => {}
            other => panic!("expected Not[And[True,False]] == True, got {other:?}"),
        }
    }
}
