//! 解释器骨架合同。

use athena_vm::{
    CancellationToken, HostOutcome, Instruction, Interpreter, SemanticOpId, SlotValue, VmConfig, VmConstant, VmExit, VmExecutor, VmHost,
    VmModule,
};

#[test]
fn empty_return_module_exits_returned() {
    let module = VmModule::empty_return();
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(vm.frames().depth(), 1);
}

#[test]
fn max_steps_budget() {
    let module = VmModule::from_instructions(
        vec![Instruction::Safepoint, Instruction::Safepoint, Instruction::Return],
        0,
    );
    let mut vm = Interpreter::new();
    let cfg = VmConfig::new().with_max_steps(1);
    let exit = vm.execute(&module, &cfg).expect("execute");
    assert_eq!(exit, VmExit::BudgetExceeded);
}

#[test]
fn cancel_at_safepoint() {
    let token = CancellationToken::new();
    token.cancel();
    let module = VmModule::from_instructions(vec![Instruction::Safepoint, Instruction::Return], 2);
    let mut vm = Interpreter::new();
    let cfg = VmConfig::new().with_cancellation(token);
    let exit = vm.execute(&module, &cfg).expect("execute");
    assert_eq!(exit, VmExit::Cancelled);
    assert_eq!(vm.slots().len(), 2);
}

#[test]
fn empty_instructions_diagnostic() {
    let module = VmModule::from_instructions(Vec::new(), 0);
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert!(matches!(exit, VmExit::Diagnostic(_)));
}

#[test]
fn load_constant_move_and_return() {
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::Move { dst: 1, src: 0 },
            Instruction::Return,
        ],
        vec![VmConstant::Boolean(true)],
        2,
    );
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(vm.slots().get(0), Some(SlotValue::Boolean(true)));
    assert_eq!(vm.slots().get(1), Some(SlotValue::Boolean(true)));
}

#[test]
fn guard_false_rejects() {
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::Guard { predicate: 0 },
            Instruction::Return,
        ],
        vec![VmConstant::Boolean(false)],
        1,
    );
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert_eq!(exit, VmExit::Rejected);
}

#[test]
fn apply_semantic_via_host() {
    struct EchoHost;
    impl VmHost for EchoHost {
        fn apply_semantic(&mut self, op: SemanticOpId, args: &[SlotValue]) -> athena_types::Result<HostOutcome> {
            assert_eq!(op.0, 7);
            assert_eq!(args, &[SlotValue::Boolean(true)]);
            Ok(HostOutcome::Value(SlotValue::Boolean(false)))
        }
    }

    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::apply_semantic1(1, SemanticOpId(7), 0),
            Instruction::Return,
        ],
        vec![VmConstant::Boolean(true)],
        2,
    );
    let mut vm = Interpreter::new();
    let mut host = EchoHost;
    let exit = vm
        .execute_with_host(&module, &VmConfig::new(), &mut host)
        .expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(vm.slots().get(1), Some(SlotValue::Boolean(false)));
}

#[test]
fn null_host_apply_semantic_diagnostics() {
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::apply_semantic1(1, SemanticOpId(1), 0),
            Instruction::Return,
        ],
        vec![VmConstant::Unit],
        2,
    );
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert!(matches!(exit, VmExit::Diagnostic(_)));
}

#[test]
fn load_term_and_symbol_constants() {
    use athena_types::{SymbolId, TermId};
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::LoadConstant { dst: 1, constant: 1 },
            Instruction::Return,
        ],
        vec![VmConstant::Term(TermId(9)), VmConstant::Symbol(SymbolId(3))],
        2,
    );
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(vm.slots().get(0), Some(SlotValue::Term(TermId(9))));
    assert_eq!(vm.slots().get(1), Some(SlotValue::Symbol(SymbolId(3))));
}

#[test]
fn branch_selects_then_or_else() {
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::Branch {
                condition: 0,
                then_pc: 2,
                else_pc: 4,
            },
            Instruction::LoadConstant { dst: 1, constant: 1 },
            Instruction::ReturnValue { slot: 1 },
            Instruction::LoadConstant { dst: 1, constant: 2 },
            Instruction::ReturnValue { slot: 1 },
        ],
        vec![
            VmConstant::Boolean(true),
            VmConstant::Boolean(true),
            VmConstant::Boolean(false),
        ],
        2,
    );
    let mut vm = Interpreter::new();
    let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(vm.last_return_slot(), Some(1));
    assert_eq!(vm.slots().get(1), Some(SlotValue::Boolean(true)));
}

#[test]
fn read_write_binding_via_host() {
    use athena_types::{BindingEvaluationPolicy, BindingKind, SymbolId};
    use std::collections::HashMap;

    struct MapHost {
        map: HashMap<u32, SlotValue>,
    }
    impl VmHost for MapHost {
        fn write_binding(
            &mut self,
            key: SlotValue,
            value: SlotValue,
            kind: BindingKind,
            evaluation: BindingEvaluationPolicy,
        ) -> athena_types::Result<HostOutcome> {
            assert_eq!(kind, BindingKind::Session);
            assert_eq!(evaluation, BindingEvaluationPolicy::EvaluateBeforeStore);
            let SlotValue::Symbol(symbol) = key else {
                panic!("expected symbol key");
            };
            self.map.insert(symbol.0, value);
            Ok(HostOutcome::Value(SlotValue::Unit))
        }

        fn read_binding(&mut self, key: SlotValue) -> athena_types::Result<HostOutcome> {
            let SlotValue::Symbol(symbol) = key else {
                panic!("expected symbol key");
            };
            Ok(HostOutcome::Value(
                self.map.get(&symbol.0).copied().unwrap_or(SlotValue::Unit),
            ))
        }
    }

    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::LoadConstant { dst: 1, constant: 1 },
            Instruction::WriteBinding {
                dst: 2,
                key: 0,
                value: 1,
                kind: BindingKind::Session,
                evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
            },
            Instruction::ReadBinding { dst: 3, key: 0 },
            Instruction::ReturnValue { slot: 3 },
        ],
        vec![VmConstant::Symbol(SymbolId(9)), VmConstant::Boolean(true)],
        4,
    );
    let mut vm = Interpreter::new();
    let mut host = MapHost {
        map: HashMap::new(),
    };
    let exit = vm
        .execute_with_host(&module, &VmConfig::new(), &mut host)
        .expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(vm.slots().get(3), Some(SlotValue::Boolean(true)));
    assert_eq!(host.map.get(&9).copied(), Some(SlotValue::Boolean(true)));
}

#[test]
fn enter_exit_scope_via_host() {
    struct ScopeHost {
        depth: u32,
    }
    impl VmHost for ScopeHost {
        fn enter_scope(&mut self, parent: Option<SlotValue>) -> athena_types::Result<HostOutcome> {
            assert!(parent.is_none());
            let d = self.depth;
            self.depth = self.depth.saturating_add(1);
            Ok(HostOutcome::Value(SlotValue::Scope(d)))
        }
        fn exit_scope(&mut self, scope: SlotValue) -> athena_types::Result<HostOutcome> {
            let SlotValue::Scope(expected) = scope else {
                panic!("expected scope");
            };
            let top = self.depth.saturating_sub(1);
            assert_eq!(expected, top);
            self.depth = top;
            Ok(HostOutcome::Value(SlotValue::Unit))
        }
    }

    let module = VmModule::from_parts(
        vec![
            Instruction::EnterScope { dst: 0, parent: None },
            Instruction::ExitScope { scope: 0 },
            Instruction::LoadConstant { dst: 1, constant: 0 },
            Instruction::ReturnValue { slot: 1 },
        ],
        vec![VmConstant::Boolean(true)],
        2,
    );
    let mut vm = Interpreter::new();
    let mut host = ScopeHost { depth: 0 };
    let exit = vm
        .execute_with_host(&module, &VmConfig::new(), &mut host)
        .expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(host.depth, 0);
    assert_eq!(vm.slots().get(1), Some(SlotValue::Boolean(true)));
}

#[test]
fn construct_collection_via_host() {
    use athena_types::CollectionKind;

    struct CollectHost {
        kind: Option<CollectionKind>,
        argc: usize,
    }
    impl VmHost for CollectHost {
        fn construct_collection(
            &mut self,
            kind: CollectionKind,
            args: &[SlotValue],
        ) -> athena_types::Result<HostOutcome> {
            self.kind = Some(kind);
            self.argc = args.len();
            assert_eq!(args, &[SlotValue::Boolean(true), SlotValue::Boolean(false)]);
            Ok(HostOutcome::Value(SlotValue::Unit))
        }
    }

    let mut args = [0u32; athena_vm::MAX_HOST_ARGS];
    args[0] = 0;
    args[1] = 1;
    let module = VmModule::from_parts(
        vec![
            Instruction::LoadConstant { dst: 0, constant: 0 },
            Instruction::LoadConstant { dst: 1, constant: 1 },
            Instruction::ConstructCollection {
                dst: 2,
                kind: CollectionKind::OrderedCollection,
                argc: 2,
                args,
            },
            Instruction::ReturnValue { slot: 2 },
        ],
        vec![VmConstant::Boolean(true), VmConstant::Boolean(false)],
        3,
    );
    let mut vm = Interpreter::new();
    let mut host = CollectHost {
        kind: None,
        argc: 0,
    };
    let exit = vm
        .execute_with_host(&module, &VmConfig::new(), &mut host)
        .expect("execute");
    assert_eq!(exit, VmExit::Returned);
    assert_eq!(host.kind, Some(CollectionKind::OrderedCollection));
    assert_eq!(host.argc, 2);
    assert_eq!(vm.slots().get(2), Some(SlotValue::Unit));
}
