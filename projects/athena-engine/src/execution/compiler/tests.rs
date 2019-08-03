    use super::*;
    use crate::{execution::reference::ReferenceExecutor, runtime::session::Session};
    use athena_types::ComputationStatus;

    #[test]
    fn compile_atom_term_module() {
        let mut session = Session::new();
        let term = session.builder().int(3, Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("atom");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("plus");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("less");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Boolean(true))) => {}
            other => panic!("expected Less[1,2,4] == True, got {other:?}"),
        }

        let x = session.builder().int(3, Default::default());
        let y = session.builder().int(1, Default::default());
        let bad = session.builder().application(less, vec![x, y], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(bad)).expect("less2");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(list)).expect("list");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(abs_term)).expect("abs");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(v))) if v.as_exact_integer() == Some(7) => {}
            other => panic!("expected Abs[-7] == 7, got {other:?}"),
        }

        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(2, Default::default());
        let list = session.builder().list(vec![a, b], Default::default());
        let length = session.operators.intern("Length");
        let length_term = session.builder().application(length, vec![list], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(length_term)).expect("length");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(first_term)).expect("first");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(a));

        let list = session.builder().list(vec![a, b, c], Default::default());
        let rest_term = session.builder().application(rest, vec![list], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(rest_term)).expect("rest");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("factorial");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("part");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(b));

        let idx_neg = session.builder().int(-1, Default::default());
        let term = session.builder().application(part, vec![list, idx_neg], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("part_neg");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(c));
    }

    #[test]
    fn compile_and_execute_span() {
        let mut session = Session::new();
        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(3, Default::default());
        let span = session.operators.intern("Span");
        let term = session.builder().application(span, vec![a, b], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("span");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("range");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(items)) if items.len() == 3 => {}
            other => panic!("expected Range[3] length 3, got {other:?}"),
        }

        let four = session.builder().int(4, Default::default());
        let sqrt = session.operators.intern("Sqrt");
        let term = session.builder().application(sqrt, vec![four], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("sqrt");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("apply");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3) => {}
            other => panic!("expected Apply[Plus, List[1,2]] == 3, got {other:?}"),
        }

        let row = session.builder().list(vec![one, two], Default::default());
        let matrix = session.builder().list(vec![row, row], Default::default());
        let size = session.operators.intern("Size");
        let term = session.builder().application(size, vec![matrix], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("size");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("map");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("zeros");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("eye");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::List(rows)) if rows.len() == 2 => {
                let expected = [[1i64, 0], [0, 1]];
                for (i, row) in rows.iter().enumerate() {
                    match session.arena.get(*row) {
                        Some(TermNode::List(cells)) if cells.len() == 2 => {
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("replace");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("simplify");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1) => {}
            other => panic!("expected Simplify[Sin[x]^2+Cos[x]^2] == 1, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_times_zero_and_cos_pi() {
        let mut session = Session::new();
        let zero = session.builder().int(0, Default::default());
        let x = session.builder().symbol("x", Default::default());
        let times = session.operators.intern("Times");
        let term = session.builder().application(times, vec![zero, x], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("times0");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0) => {}
            other => panic!("expected Times[0,x] == 0, got {other:?}"),
        }

        let pi = session.builder().symbol("Pi", Default::default());
        let cos = session.operators.intern("Cos");
        let term = session.builder().application(cos, vec![pi], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("cos");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let power = session.operators.intern("Power");
        let times = session.operators.intern("Times");
        let pow = session.builder().application(power, vec![x, zero], Default::default());
        let term = session.builder().application(times, vec![two, pow], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("power0");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
            other => panic!("expected Times[2, Power[x,0]] == 2, got {other:?}"),
        }

        let one = session.builder().int(1, Default::default());
        let cosh = session.operators.intern("Cosh");
        let cosh_x = session.builder().application(cosh, vec![x], Default::default());
        let term = session.builder().application(times, vec![cosh_x, one], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("cosh");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Application { head, arguments })
                if session.operators.name(*head) == Some("Cosh") && arguments.len() == 1 && session.arena.structural_eq(arguments[0], x) => {}
            other => panic!("expected Times[Cosh[x], 1] == Cosh[x], got {other:?}"),
        }

        let neg1 = session.builder().int(-1, Default::default());
        let two = session.builder().int(2, Default::default());
        let inner = session.builder().application(power, vec![x, neg1], Default::default());
        let nested = session.builder().application(power, vec![inner, two], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(nested)).expect("nested power");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Application { head, arguments })
                if session.operators.name(*head) == Some("Power")
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
        let times = session.operators.intern("Times");
        let plus = session.operators.intern("Plus");
        let t1 = session.builder().application(times, vec![two, x], Default::default());
        let t2 = session.builder().application(times, vec![three, x], Default::default());
        let sum = session.builder().application(plus, vec![t1, t2], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(sum)).expect("like plus");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Application { head, arguments })
                if session.operators.name(*head) == Some("Times")
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
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        // 2*(x+1) → 2x+2
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Application { head, arguments }) if session.operators.name(*head) == Some("Plus") && arguments.len() == 2 => {}
            other => panic!("expected distribute to Plus, got {other:?}"),
        }
    }

    #[test]
    fn compile_unknown_head_stays_residual() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", Default::default());
        let head = session.operators.intern("Foo");
        let term = session.builder().application(head, vec![x], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("foo");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Application { head, arguments })
                if session.operators.name(*head) == Some("Foo") && arguments.len() == 1 && session.arena.structural_eq(arguments[0], x) => {}
            other => panic!("expected Foo[x] residual, got {other:?}"),
        }
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
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("branch");
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
        let request = AthenaRequest::Command(SessionCommand::Define { symbol, value, timing: DefinitionEvaluationTiming::Immediate });
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("define");
        assert!(!module.effect_edges.is_empty());
        ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.defs.own(symbol), Some(value));
    }

    #[test]
    fn compile_and_execute_define_deferred_evaluates_on_read() {
        let mut session = Session::new();
        let plus = session.operators.intern("Plus");
        let a = session.builder().int(1, Default::default());
        let b = session.builder().int(1, Default::default());
        let rhs = session.builder().application(plus, vec![a, b], Default::default());
        let head = session.operators.intern("DefineDeferred");
        let sym_term = session.builder().symbol("a", Default::default());
        let term = session.builder().application(head, vec![sym_term, rhs], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("define deferred");
        ReferenceExecutor::new().execute(&mut session, &module, None).expect("define exec");
        let symbol = match session.arena.get(sym_term) {
            Some(TermNode::Atom(Atom::Symbol(id))) => *id,
            other => panic!("expected symbol, got {other:?}"),
        };
        assert!(session.defs.own(symbol).is_none());
        assert_eq!(session.defs.delayed(symbol), Some(rhs));

        let read_module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(sym_term)).expect("read");
        let result_id = ReferenceExecutor::new().execute(&mut session, &read_module, None).expect("read exec");
        let loaded = session.results.get(result_id).expect("result");
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
            other => panic!("expected delayed Plus[1,1] == 2, got {other:?}"),
        }
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
        let define = AthenaRequest::Command(SessionCommand::Define { symbol, value, timing: DefinitionEvaluationTiming::Immediate });
        let define_module = ExecutionCompiler::new().compile(&mut session, &define).expect("define");
        ReferenceExecutor::new().execute(&mut session, &define_module, None).expect("define exec");

        let read = AthenaRequest::Term(sym_term);
        let read_module = ExecutionCompiler::new().compile(&mut session, &read).expect("read");
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
                AthenaRequest::Command(SessionCommand::Define { symbol, value, timing: DefinitionEvaluationTiming::Immediate }),
                AthenaRequest::Term(sym_term),
                AthenaRequest::Command(SessionCommand::ClearDefinition { symbol }),
            ],
        });
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("sequence");
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
        let request =
            AthenaRequest::Control(ControlPlan::CountedLoop { variable: var, iterator: iter, body: Box::new(AthenaRequest::Term(var)) });
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("counted");
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
    fn compile_and_execute_term_counted_loop_span() {
        let mut session = Session::new();
        let var = session.builder().symbol("i", Default::default());
        let one = session.builder().int(1, Default::default());
        let three = session.builder().int(3, Default::default());
        let span = session.operators.intern("Span");
        let loop_op = session.operators.intern("CountedLoop");
        let iter = session.builder().application(span, vec![one, three], Default::default());
        let term = session.builder().application(loop_op, vec![var, iter, var], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("counted span");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3) => {}
            other => panic!("expected CountedLoop[i, Span[1,3], i] == 3, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_loop_while_false() {
        let mut session = Session::new();
        let cond = session.builder().boolean(false, Default::default());
        let body = session.builder().int(1, Default::default());
        let request = AthenaRequest::Control(ControlPlan::LoopWhile { condition: cond, body: Box::new(AthenaRequest::Term(body)) });
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("loop");
        assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::BudgetCheck)));
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        match session.arena.get(loaded.symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Null)) => {}
            other => panic!("expected Unit/Null after zero-trip loop, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_term_loop_while_zero() {
        let mut session = Session::new();
        let zero = session.builder().int(0, Default::default());
        let body = session.builder().int(1, Default::default());
        let head = session.operators.intern("LoopWhile");
        let term = session.builder().application(head, vec![zero, body], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("loop term");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        match session.arena.get(loaded.symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Null)) => {}
            other => panic!("expected Null after LoopWhile[0,1], got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_goal_call_provider_dispatches_domain() {
        use crate::{
            api::request::DomainGoal,
            domains::{dispatch::DomainRequest, number_theory::NumberTheoryRequest},
            execution::execute_ir_request,
            runtime::values::RuntimeValue,
        };
        use athena_numeric::Integer;

        let mut session = Session::new();
        let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
            a: Integer::from_i64(12),
            b: Integer::from_i64(8),
        })));
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("goal");
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
        use crate::{
            api::request::DomainGoal,
            domains::{dispatch::DomainRequest, number_theory::NumberTheoryRequest},
        };
        use athena_numeric::Integer;

        let mut session = Session::new();
        let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
            a: Integer::from_i64(12),
            b: Integer::from_i64(8),
        })));
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("goal");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
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
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("recover");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(body));
    }

    #[test]
    fn compile_and_execute_term_recover_error_and_success() {
        let mut session = Session::new();
        let recover = session.operators.intern("Recover");
        let error = session.operators.intern("error");
        let msg = session.builder().string("e", Default::default());
        let err_body = session.builder().application(error, vec![msg], Default::default());
        let one = session.builder().int(1, Default::default());
        let err_term = session.builder().application(recover, vec![err_body, one], Default::default());
        let err_mod = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(err_term)).expect("recover err");
        let err_id = ReferenceExecutor::new().execute(&mut session, &err_mod, None).expect("err exec");
        let err_out = session.results.get(err_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(err_out) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1) => {}
            other => panic!("expected Recover[error,1] == 1, got {other:?}"),
        }

        let two = session.builder().int(2, Default::default());
        let three = session.builder().int(3, Default::default());
        let ok_term = session.builder().application(recover, vec![two, three], Default::default());
        let ok_mod = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(ok_term)).expect("recover ok");
        let ok_id = ReferenceExecutor::new().execute(&mut session, &ok_mod, None).expect("ok exec");
        let ok_out = session.results.get(ok_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(ok_out) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
            other => panic!("expected Recover[2,3] == 2, got {other:?}"),
        }
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
            arms: vec![(c0, Box::new(AthenaRequest::Term(a0))), (c1, Box::new(AthenaRequest::Term(a1)))],
            otherwise: Some(Box::new(AthenaRequest::Term(otherwise))),
        });
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("cond");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(a1));
    }

    #[test]
    fn compile_and_execute_local_scope_body() {
        let mut session = Session::new();
        let term = session.builder().int(11, Default::default());
        let request = AthenaRequest::Control(ControlPlan::LocalScope { body: Box::new(AthenaRequest::Term(term)) });
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("scope");
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
                    AthenaRequest::Command(SessionCommand::Define { symbol, value: local, timing: DefinitionEvaluationTiming::Immediate }),
                    AthenaRequest::Term(sym_term),
                ],
            })),
        });
        let module = ExecutionCompiler::new().compile(&mut session, &request).expect("scope");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(local));
        // Session Own unchanged after local scope exits.
        assert_eq!(session.defs.own(symbol), Some(global));
    }

    #[test]
    fn compile_and_execute_term_local_scope_with_define() {
        let mut session = Session::new();
        let define = session.operators.intern("Define");
        let plus = session.operators.intern("Plus");
        let scope = session.operators.intern("LocalScope");
        let x = session.builder().symbol("x", Default::default());
        let one = session.builder().int(1, Default::default());
        let def = session.builder().application(define, vec![x, one], Default::default());
        let locals = session.builder().list(vec![def], Default::default());
        let one2 = session.builder().int(1, Default::default());
        let body = session.builder().application(plus, vec![x, one2], Default::default());
        let term = session.builder().application(scope, vec![locals, body], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("local scope");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
            other => panic!("expected LocalScope Define body == 2, got {other:?}"),
        }
        let symbol = match session.arena.get(x) {
            Some(TermNode::Atom(Atom::Symbol(id))) => *id,
            other => panic!("expected symbol, got {other:?}"),
        };
        assert!(session.defs.own(symbol).is_none());
    }

    #[test]
    fn compile_and_execute_term_lexical_scope_bare_unique() {
        let mut session = Session::new();
        let scope = session.operators.intern("LexicalScope");
        let x = session.builder().symbol("x", Default::default());
        let locals = session.builder().list(vec![x], Default::default());
        let term = session.builder().application(scope, vec![locals, x], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("lexical");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        let out = loaded.symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::Atom(Atom::Symbol(sym))) => {
                let name = session.arena.symbols().resolve(*sym).unwrap_or("");
                assert!(name.starts_with("x$"), "got {name}");
            }
            other => panic!("expected unique x$N, got {other:?}"),
        }
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("bool ops");
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
        let term = session.builder().application(if_op, vec![cond, then_term, else_term], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("if");
        assert_eq!(module.regions[0].blocks.len(), 3);
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(then_term));
    }

    #[test]
    fn compile_and_execute_sequence_and_hold() {
        let mut session = Session::new();
        let one = session.builder().int(1, Default::default());
        let two = session.builder().int(2, Default::default());
        let three = session.builder().int(3, Default::default());
        let seq = session.operators.intern("Sequence");
        let term = session.builder().application(seq, vec![one, two, three], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("sequence");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(three));

        let plus = session.operators.intern("Plus");
        let hold = session.operators.intern("Hold");
        let inner = session.builder().application(plus, vec![one, one], Default::default());
        let held = session.builder().application(hold, vec![inner], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(held)).expect("hold");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
        match session.arena.get(out) {
            Some(TermNode::Application { head, arguments })
                if session.operators.name(*head) == Some("Hold")
                    && arguments.len() == 1
                    && session.arena.structural_eq(arguments[0], inner) => {}
            other => panic!("expected Hold[Plus[1,1]] unevaluated, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_part_end_and_cond() {
        let mut session = Session::new();
        let one = session.builder().int(1, Default::default());
        let two = session.builder().int(2, Default::default());
        let three = session.builder().int(3, Default::default());
        let list = session.builder().list(vec![one, two, three], Default::default());
        let end = session.builder().symbol("End", Default::default());
        let part = session.operators.intern("Part");
        let term = session.builder().application(part, vec![list, end], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("part end");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(three));

        let fals = session.builder().symbol("False", Default::default());
        let tru = session.builder().symbol("True", Default::default());
        let cond = session.operators.intern("Cond");
        let term = session.builder().application(cond, vec![fals, one, tru, two, tru, three], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("cond");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(two));
    }

    #[test]
    fn compile_and_execute_term_define_in_sequence() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", Default::default());
        let five = session.builder().int(5, Default::default());
        let one = session.builder().int(1, Default::default());
        let define = session.operators.intern("Define");
        let plus = session.operators.intern("Plus");
        let seq = session.operators.intern("Sequence");
        let def = session.builder().application(define, vec![x, five], Default::default());
        let use_x = session.builder().application(plus, vec![x, one], Default::default());
        let term = session.builder().application(seq, vec![def, use_x], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("define seq");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(6) => {}
            other => panic!("expected Sequence[Define[x,5], Plus[x,1]] == 6, got {other:?}"),
        }
    }

    #[test]
    fn compile_and_execute_runtime_branch() {
        let mut session = Session::new();
        let one = session.builder().int(1, Default::default());
        let seven = session.builder().int(7, Default::default());
        let eight = session.builder().int(8, Default::default());
        let equal = session.operators.intern("Equal");
        let branch = session.operators.intern("Branch");
        let cond = session.builder().application(equal, vec![one, one], Default::default());
        let term = session.builder().application(branch, vec![cond, seven, eight], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("branch");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(seven));

        let fals = session.builder().symbol("False", Default::default());
        let term = session.builder().application(branch, vec![fals, seven, eight], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("branch false");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(eight));
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
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("sameq");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        match session.arena.get(loaded.symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Boolean(false))) => {}
            other => panic!("expected False, got {other:?}"),
        }

        let a = session.builder().int(3, Default::default());
        let b = session.builder().int(3, Default::default());
        let eq = session.operators.intern("Equal");
        let eq_term = session.builder().application(eq, vec![a, b], Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(eq_term)).expect("equal");
        let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
        let loaded = session.results.get(result_id).expect("result");
        match session.arena.get(loaded.symbolic_term.expect("term")) {
            Some(TermNode::Atom(Atom::Boolean(true))) => {}
            other => panic!("expected Equal[3,3] == True, got {other:?}"),
        }
    }
