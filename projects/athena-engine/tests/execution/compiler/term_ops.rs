use athena_engine::{
    Session,
    api::request::{AthenaRequest, ControlPlan, SessionCommand},
    execution::{
        compiler::ExecutionCompiler,
        ir::{CapturedRoot, EffectKind, OperationKind},
        reference::ReferenceExecutor,
    },
};
use athena_ir::{ApplicationHead, Atom, MathematicalConstant, SemanticOperator, TermNode, UnaryFunction};
use athena_types::ComputationStatus;

#[test]
fn compile_atom_term_module() {
    let mut session = Session::new();
    let term = session.builder().int(3, Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("atom");
    assert_eq!(module.captured_roots, vec![CapturedRoot::term(session.arena.term_ref(term).expect("term_ref"))]);
    assert_eq!(module.regions.len(), 1);
}

#[test]
fn compile_and_execute_plus_integers() {
    let mut session = Session::new();
    let a = session.builder().int(2, Default::default());
    let b = session.builder().int(3, Default::default());
    let plus = ApplicationHead::Semantic(SemanticOperator::Add);
    let term = session.builder().application(plus, vec![a, b], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("plus");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
    let less = ApplicationHead::Semantic(SemanticOperator::Less);
    let term = session.builder().application(less, vec![a, b, c], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("less");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Boolean(true))) => {}
        other => panic!("expected Less[1,2,4] == True, got {other:?}"),
    }

    let x = session.builder().int(3, Default::default());
    let y = session.builder().int(1, Default::default());
    let bad = session.builder().application(less, vec![x, y], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(bad)).expect("less2");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
    let plus = ApplicationHead::Semantic(SemanticOperator::Add);
    let sum = session.builder().application(plus, vec![a, b], Default::default());
    let c = session.builder().int(9, Default::default());
    let list = session.builder().list(vec![sum, c], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(list)).expect("list");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
            match session.arena.get(items[0]) {
                Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5) => {}
                other => panic!("expected first element 5, got {other:?}"),
            }
            assert_eq!(items[1], c);
        }
        other => panic!("expected OrderedCollection[5,9], got {other:?}"),
    }
}

#[test]
fn compile_and_execute_abs_and_length() {
    let mut session = Session::new();
    let n = session.builder().int(-7, Default::default());
    let abs = ApplicationHead::Semantic(SemanticOperator::Abs);
    let abs_term = session.builder().application(abs, vec![n], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(abs_term)).expect("abs");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(v))) if v.as_exact_integer() == Some(7) => {}
        other => panic!("expected Abs[-7] == 7, got {other:?}"),
    }

    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let list = session.builder().list(vec![a, b], Default::default());
    let length = ApplicationHead::Semantic(SemanticOperator::Length);
    let length_term = session.builder().application(length, vec![list], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(length_term)).expect("length");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(v))) if v.as_exact_integer() == Some(2) => {}
        other => panic!("expected Length[OrderedCollection[1,2]] == 2, got {other:?}"),
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
    let join = ApplicationHead::Semantic(SemanticOperator::Join);
    let joined = session.builder().application(join, vec![left, right], Default::default());
    let first = ApplicationHead::Semantic(SemanticOperator::First);
    let rest = ApplicationHead::Semantic(SemanticOperator::Rest);
    let first_term = session.builder().application(first, vec![joined], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(first_term)).expect("first");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(a));

    let list = session.builder().list(vec![a, b, c], Default::default());
    let rest_term = session.builder().application(rest, vec![list], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(rest_term)).expect("rest");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.as_slice() == [b, c] => {}
        other => panic!("expected Rest == OrderedCollection[2,3], got {other:?}"),
    }
}

#[test]
fn compile_and_execute_most_and_reverse() {
    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let list = session.builder().list(vec![a, b, c], Default::default());

    let most_term = session
        .builder()
        .application_semantic(SemanticOperator::Most, vec![list], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(most_term)).expect("most");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.as_slice() == [a, b] => {}
        other => panic!("expected Most == OrderedCollection[1,2], got {other:?}"),
    }

    let rev_term = session
        .builder()
        .application_semantic(SemanticOperator::Reverse, vec![list], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(rev_term)).expect("reverse");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.as_slice() == [c, b, a] => {}
        other => panic!("expected Reverse == OrderedCollection[3,2,1], got {other:?}"),
    }
}

#[test]
fn compile_and_execute_take_and_drop() {
    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let d = session.builder().int(4, Default::default());
    let list = session.builder().list(vec![a, b, c, d], Default::default());
    let two = session.builder().int(2, Default::default());

    let take_term = session
        .builder()
        .application_semantic(SemanticOperator::Take, vec![list, two], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(take_term)).expect("take");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.as_slice() == [a, b] => {}
        other => panic!("expected Take == OrderedCollection[1,2], got {other:?}"),
    }

    let drop_term = session
        .builder()
        .application_semantic(SemanticOperator::Drop, vec![list, two], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(drop_term)).expect("drop");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.as_slice() == [c, d] => {}
        other => panic!("expected Drop == OrderedCollection[3,4], got {other:?}"),
    }
}

#[test]
fn compile_and_execute_head_of_add_and_list() {
    use athena_engine::runtime::values::arena::symbol_name;

    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let sum = session.builder().application_semantic(SemanticOperator::Add, vec![one, two], Default::default());
    let head_sum = session
        .builder()
        .application_semantic(SemanticOperator::Head, vec![sum], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(head_sum)).expect("head add");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Add),
            arguments,
        }) if arguments.is_empty() => {}
        other => panic!("expected 0-ary Add from Head[1+2], got {other:?}"),
    }

    let list = session.builder().list(vec![one, two], Default::default());
    let head_list = session
        .builder()
        .application_semantic(SemanticOperator::Head, vec![list], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(head_list)).expect("head list");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    assert_eq!(symbol_name(&session, out).as_deref(), Some("List"));
}

#[test]
fn compile_and_execute_factorial() {
    let mut session = Session::new();
    let n = session.builder().int(5, Default::default());
    let fact = ApplicationHead::Semantic(SemanticOperator::Factorial);
    let term = session.builder().application(fact, vec![n], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("factorial");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(v))) if v.as_exact_integer() == Some(120) => {}
        other => panic!("expected Factorial[5] == 120, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_range_and_sqrt() {
    let mut session = Session::new();
    let n = session.builder().int(3, Default::default());
    let range = ApplicationHead::Semantic(SemanticOperator::Range);
    let term = session.builder().application(range, vec![n], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("range");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.len() == 3 => {}
        other => panic!("expected Range[3] length 3, got {other:?}"),
    }

    let four = session.builder().int(4, Default::default());
    let sqrt = ApplicationHead::Semantic(SemanticOperator::Sqrt);
    let term = session.builder().application(sqrt, vec![four], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("sqrt");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
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
    let plus = session.builder().application_semantic(SemanticOperator::Add, vec![], Default::default());
    let apply = ApplicationHead::Semantic(SemanticOperator::Apply);
    let term = session.builder().application(apply, vec![plus, list], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("apply");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3) => {}
        other => panic!("expected Apply[Add, OrderedCollection[1,2]] == 3, got {other:?}"),
    }

    let row = session.builder().list(vec![one, two], Default::default());
    let matrix = session.builder().list(vec![row, row], Default::default());
    let size = ApplicationHead::Semantic(SemanticOperator::Size);
    let term = session.builder().application(size, vec![matrix], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("size");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
            for (i, expected) in [2i64, 2].into_iter().enumerate() {
                match session.arena.get(items[i]) {
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(expected) => {}
                    other => panic!("expected Size dim {expected}, got {other:?}"),
                }
            }
        }
        other => panic!("expected Size == OrderedCollection[2,2], got {other:?}"),
    }
}

#[test]
fn compile_and_execute_map_symbol() {
    let mut session = Session::new();
    let a = session.builder().int(-1, Default::default());
    let b = session.builder().int(4, Default::default());
    let list = session.builder().list(vec![a, b], Default::default());
    let abs = session.builder().application_semantic(SemanticOperator::Abs, vec![], Default::default());
    let map = ApplicationHead::Semantic(SemanticOperator::Map);
    let term = session.builder().application(map, vec![abs, list], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("map");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
            match session.arena.get(items[0]) {
                Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1) => {}
                other => panic!("expected Abs[-1]==1, got {other:?}"),
            }
            match session.arena.get(items[1]) {
                Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(4) => {}
                other => panic!("expected Abs[4]==4, got {other:?}"),
            }
        }
        other => panic!("expected Map[Abs, OrderedCollection[-1,4]] == OrderedCollection[1,4], got {other:?}"),
    }
}

#[test]
fn compile_and_execute_map_indexed_second_slot() {
    // MapIndexed[Function[{s1,s2}, s2], {a,b}] → {{1},{2}}
    let mut session = Session::new();
    let a = session.builder().symbol("a", Default::default());
    let b = session.builder().symbol("b", Default::default());
    let list = session.builder().list(vec![a, b], Default::default());
    let s1 = session.builder().symbol("$slot1", Default::default());
    let s2 = session.builder().symbol("$slot2", Default::default());
    let binders = session.builder().list(vec![s1, s2], Default::default());
    let func = session
        .builder()
        .application_semantic(SemanticOperator::Function, vec![binders, s2], Default::default());
    let term = session
        .builder()
        .application_semantic(SemanticOperator::MapIndexed, vec![func, list], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("mapindexed");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
            for (i, item) in items.iter().enumerate() {
                match session.arena.get(*item) {
                    Some(TermNode::Collection { elements: idx, .. }) if idx.len() == 1 => {
                        match session.arena.get(idx[0]) {
                            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some((i as i64) + 1) => {}
                            other => panic!("expected index {}, got {other:?}", i + 1),
                        }
                    }
                    other => panic!("expected singleton index list, got {other:?}"),
                }
            }
        }
        other => panic!("expected MapIndexed index lists, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_map_thread_plus() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let four = session.builder().int(4, Default::default());
    let left = session.builder().list(vec![one, two], Default::default());
    let right = session.builder().list(vec![three, four], Default::default());
    let lists = session.builder().list(vec![left, right], Default::default());
    let plus = session.builder().application_semantic(SemanticOperator::Add, vec![], Default::default());
    let term = session
        .builder()
        .application_semantic(SemanticOperator::MapThread, vec![plus, lists], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("mapthread");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
            match session.arena.get(items[0]) {
                Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(4) => {}
                other => panic!("expected 1+3==4, got {other:?}"),
            }
            match session.arena.get(items[1]) {
                Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(6) => {}
                other => panic!("expected 2+4==6, got {other:?}"),
            }
        }
        other => panic!("expected MapThread Plus results, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_zeros_eye() {
    let mut session = Session::new();
    let two = session.builder().int(2, Default::default());
    let zeros = ApplicationHead::Semantic(SemanticOperator::Zeros);
    let term = session.builder().application(zeros, vec![two], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("zeros");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: rows, .. }) if rows.len() == 2 => {
            for row in rows {
                match session.arena.get(*row) {
                    Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 2 => {
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

    let eye = ApplicationHead::Semantic(SemanticOperator::Eye);
    let term = session.builder().application(eye, vec![two], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("eye");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: rows, .. }) if rows.len() == 2 => {
            let expected = [[1i64, 0], [0, 1]];
            for (i, row) in rows.iter().enumerate() {
                match session.arena.get(*row) {
                    Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 2 => {
                        for (j, cell) in cells.iter().enumerate() {
                            match session.arena.get(*cell) {
                                Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(expected[i][j]) => {}
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
fn compile_and_execute_diagonal_matrix() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let diag = session.builder().list(vec![one, two], Default::default());
    let head = ApplicationHead::Semantic(SemanticOperator::DiagonalMatrix);
    let term = session.builder().application(head, vec![diag], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("diag");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: rows, .. }) if rows.len() == 2 => {
            let expected = [[1i64, 0], [0, 2]];
            for (i, row) in rows.iter().enumerate() {
                match session.arena.get(*row) {
                    Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 2 => {
                        for (j, cell) in cells.iter().enumerate() {
                            match session.arena.get(*cell) {
                                Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(expected[i][j]) => {}
                                other => panic!("expected DiagonalMatrix[{i},{j}]={}, got {other:?}", expected[i][j]),
                            }
                        }
                    }
                    other => panic!("expected DiagonalMatrix row, got {other:?}"),
                }
            }
        }
        other => panic!("expected DiagonalMatrix[{{1,2}}], got {other:?}"),
    }
}

#[test]
fn compile_and_execute_replace_all() {
    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let plus = ApplicationHead::Semantic(SemanticOperator::Add);
    let expr = session.builder().application(plus, vec![x, one], Default::default());
    let rule_op = ApplicationHead::Semantic(SemanticOperator::Rule);
    let rule = session.builder().application(rule_op, vec![x, two], Default::default());
    let replace = ApplicationHead::Semantic(SemanticOperator::ReplaceAll);
    let term = session.builder().application(replace, vec![expr, rule], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("replace");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3) => {}
        other => panic!("expected ReplaceAll[Plus[x,1], x->2] == 3, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_simplify_pythagorean() {
    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let sin = ApplicationHead::Semantic(SemanticOperator::from_unary(UnaryFunction::Sin));
    let cos = ApplicationHead::Semantic(SemanticOperator::from_unary(UnaryFunction::Cos));
    let power = ApplicationHead::Semantic(SemanticOperator::Power);
    let plus = ApplicationHead::Semantic(SemanticOperator::Add);
    let two = session.builder().int(2, Default::default());
    let sin_x = session.builder().application(sin, vec![x], Default::default());
    let cos_x = session.builder().application(cos, vec![x], Default::default());
    let sin2 = session.builder().application(power, vec![sin_x, two], Default::default());
    let cos2 = session.builder().application(power, vec![cos_x, two], Default::default());
    let sum = session.builder().application(plus, vec![sin2, cos2], Default::default());
    let simplify = ApplicationHead::Semantic(SemanticOperator::Simplify);
    let term = session.builder().application(simplify, vec![sum], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("simplify");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1) => {}
        other => panic!("expected Simplify[Sin[x]^2+Cos[x]^2] == 1, got {other:?}"),
    }
}

#[test]
fn simplify_does_not_reapply_ambient_own_to_free_symbol() {
    use athena_engine::api::{AthenaEngine, request::SessionCommand};
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let symbol = match session.arena.get(x) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let five = session.builder().int(5, Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value: five,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let define_module = ExecutionCompiler::new().compile(&mut session, &define).expect("define");
    ReferenceExecutor::new().execute(&mut session, &define_module).expect("define exec");

    let engine = AthenaEngine::new();
    let simplified = engine.simplify(&mut session, x);
    assert!(
        session.arena.structural_eq(simplified, x),
        "Simplify of a free symbol must not substitute ambient Own (result transform contract)"
    );
}

#[test]
fn compile_and_execute_times_zero_and_cos_pi() {
    let mut session = Session::new();
    let zero = session.builder().int(0, Default::default());
    let x = session.builder().symbol("x", Default::default());
    let times = ApplicationHead::Semantic(SemanticOperator::Multiply);
    let term = session.builder().application(times, vec![zero, x], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("times0");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0) => {}
        other => panic!("expected Times[0,x] == 0, got {other:?}"),
    }

    let pi = session.builder().constant(MathematicalConstant::Pi, Default::default());
    let cos = ApplicationHead::Semantic(SemanticOperator::from_unary(UnaryFunction::Cos));
    let term = session.builder().application(cos, vec![pi], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("cos");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(-1) => {}
        other => panic!("expected Cos[Pi] == -1, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_power_zero_and_times_one_residual() {
    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let zero = session.builder().int(0, Default::default());
    let two = session.builder().int(2, Default::default());
    let power = ApplicationHead::Semantic(SemanticOperator::Power);
    let times = ApplicationHead::Semantic(SemanticOperator::Multiply);
    let pow = session.builder().application(power, vec![x, zero], Default::default());
    let term = session.builder().application(times, vec![two, pow], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("power0");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
        other => panic!("expected Times[2, Power[x,0]] == 2, got {other:?}"),
    }

    let one = session.builder().int(1, Default::default());
    let cosh = ApplicationHead::Semantic(SemanticOperator::from_unary(UnaryFunction::Cosh));
    let cosh_x = session.builder().application(cosh, vec![x], Default::default());
    let term = session.builder().application(times, vec![cosh_x, one], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("cosh");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Application { head, arguments })
            if matches!(
                *head,
                ApplicationHead::Semantic(op) if op.as_unary() == Some(UnaryFunction::Cosh)
            ) && arguments.len() == 1
                && session.arena.structural_eq(arguments[0], x) => {}
        other => panic!("expected Times[Cosh[x], 1] == Cosh[x], got {other:?}"),
    }

    let neg1 = session.builder().int(-1, Default::default());
    let two = session.builder().int(2, Default::default());
    let inner = session.builder().application(power, vec![x, neg1], Default::default());
    let nested = session.builder().application(power, vec![inner, two], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(nested)).expect("nested power");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Application { head, arguments })
            if matches!(*head, ApplicationHead::Semantic(SemanticOperator::Power))
                && arguments.len() == 2
                && session.arena.structural_eq(arguments[0], x)
                && matches!(
                    session.arena.get(arguments[1]),
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(-2)
                ) => {}
        other => panic!("expected (x^-1)^2 == x^-2, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_plus_like_terms_and_distribute() {
    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let times = ApplicationHead::Semantic(SemanticOperator::Multiply);
    let plus = ApplicationHead::Semantic(SemanticOperator::Add);
    let t1 = session.builder().application(times, vec![two, x], Default::default());
    let t2 = session.builder().application(times, vec![three, x], Default::default());
    let sum = session.builder().application(plus, vec![t1, t2], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(sum)).expect("like plus");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Application { head, arguments })
            if matches!(*head, ApplicationHead::Semantic(SemanticOperator::Multiply))
                && arguments.len() == 2
                && matches!(
                    session.arena.get(arguments[0]),
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5)
                )
                && session.arena.structural_eq(arguments[1], x) => {}
        other => panic!("expected 2x+3x == 5x, got {other:?}"),
    }

    let one = session.builder().int(1, Default::default());
    let inner = session.builder().application(plus, vec![x, one], Default::default());
    let dist = session.builder().application(times, vec![two, inner], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(dist)).expect("distribute");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    // 2*(x+1) → 2x+2
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Application { head, arguments })
            if matches!(*head, ApplicationHead::Semantic(SemanticOperator::Add)) && arguments.len() == 2 => {}
        other => panic!("expected distribute to Plus, got {other:?}"),
    }
}

#[test]
fn compile_unknown_head_stays_residual() {
    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let head = ApplicationHead::Extension(session.extensions.intern("Foo"));
    let term = session.builder().application(head, vec![x], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("foo");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Application { head, arguments })
            if matches!(*head, ApplicationHead::Extension(id) if session.extensions.display_name(id) == Some("Foo"))
                && arguments.len() == 1
                && session.arena.structural_eq(arguments[0], x) => {}
        other => panic!("expected Foo[x] residual, got {other:?}"),
    }
}

fn assert_indeterminate(session: &Session, term_id: athena_types::TermId) {
    match session.arena.get(term_id) {
        Some(TermNode::Application { head, arguments })
            if matches!(*head, ApplicationHead::Semantic(SemanticOperator::Indeterminate)) && arguments.is_empty() => {}
        other => panic!("expected Indeterminate[], got {other:?}"),
    }
}

#[test]
fn singular_forms_fold_to_indeterminate() {
    let mut session = Session::new();
    let zero = session.builder().int(0, Default::default());
    let divide = ApplicationHead::Semantic(SemanticOperator::Divide);
    let power = ApplicationHead::Semantic(SemanticOperator::Power);
    let subtract = ApplicationHead::Semantic(SemanticOperator::Subtract);

    let div = session.builder().application(divide, vec![zero, zero], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(div)).expect("0/0");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_indeterminate(&session, session.results.get(result_id).expect("result").symbolic_term.expect("term"));

    let pow = session.builder().application(power, vec![zero, zero], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(pow)).expect("0^0");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_indeterminate(&session, session.results.get(result_id).expect("result").symbolic_term.expect("term"));

    let infinity = session.builder().constant(MathematicalConstant::Infinity, Default::default());
    let inf_minus_inf = session.builder().application(subtract, vec![infinity, infinity], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(inf_minus_inf)).expect("inf-inf");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_indeterminate(&session, session.results.get(result_id).expect("result").symbolic_term.expect("term"));
}

#[test]
fn nonzero_over_zero_keeps_divide_residual() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let zero = session.builder().int(0, Default::default());
    let divide = ApplicationHead::Semantic(SemanticOperator::Divide);
    let term = session.builder().application(divide, vec![one, zero], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("1/0");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Application { head, arguments })
            if matches!(*head, ApplicationHead::Semantic(SemanticOperator::Divide))
                && arguments.len() == 2
                && matches!(
                    session.arena.get(arguments[0]),
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)
                )
                && matches!(
                    session.arena.get(arguments[1]),
                    Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)
                ) => {}
        other => panic!("expected Divide[1,0] residual, got {other:?}"),
    }
}
