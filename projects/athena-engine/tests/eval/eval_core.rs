//! 桥接 `evaluate` 覆盖（集成测试位于 crate 根旁）。

use athena_engine::{Atom, Term, clone_term, evaluate, evaluate_checked};

#[test]
fn plus_fold() {
    let e = evaluate(&Term::apply("Plus", vec![Term::int(1), Term::int(2), Term::symbol("x")]));
    assert_eq!(e, Term::apply("Plus", vec![Term::int(3), Term::symbol("x")]));
}

#[test]
fn power_one() {
    let e = evaluate(&Term::apply("Power", vec![Term::symbol("x"), Term::int(1)]));
    assert_eq!(e, Term::symbol("x"));
}

#[test]
fn list_eval() {
    let e = evaluate(&Term::List(vec![Term::int(1), Term::apply("Plus", vec![Term::int(2), Term::int(2)])]));
    assert_eq!(e, Term::List(vec![Term::int(1), Term::int(4)]));
}

#[test]
fn d_power() {
    let e = evaluate(&Term::apply("D", vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(3)]), Term::symbol("x")]));
    assert!(matches!(e, Term::Application { .. }));
    let text = format!("{e:?}");
    assert!(text.contains("x"), "got {text}");
}

#[test]
fn pythagorean() {
    let sin2 = Term::apply("Power", vec![Term::apply("Sin", vec![Term::symbol("x")]), Term::int(2)]);
    let cos2 = Term::apply("Power", vec![Term::apply("Cos", vec![Term::symbol("x")]), Term::int(2)]);
    let e = evaluate(&Term::apply("Simplify", vec![Term::apply("Plus", vec![sin2, cos2])]));
    assert_eq!(e, Term::int(1));
}

#[test]
fn compound_expression_returns_last() {
    let e = evaluate(&Term::apply("CompoundExpression", vec![Term::int(1), Term::int(2), Term::int(3)]));
    assert_eq!(e, Term::int(3));
}

#[test]
fn integrate_power() {
    let e = evaluate(&Term::apply(
        "Integrate",
        vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]), Term::symbol("x")],
    ));
    let text = format!("{e:?}");
    assert!(text.contains("x"), "got {text}");
}

#[test]
fn map_sin_list() {
    let e = evaluate(&Term::apply("Map", vec![Term::symbol("Sin"), Term::List(vec![Term::int(0)])]));
    assert!(matches!(e, Term::List(_)));
}

#[test]
fn truthy_via_and_or() {
    assert_eq!(evaluate(&Term::apply("And", vec![Term::int(0), Term::int(1)])), Term::boolean(false));
    assert_eq!(evaluate(&Term::apply("And", vec![Term::int(1), Term::int(1)])), Term::boolean(true));
    assert_eq!(evaluate(&Term::apply("Or", vec![Term::int(0), Term::int(0)])), Term::boolean(false));
    assert_eq!(evaluate(&Term::apply("Or", vec![Term::int(0), Term::int(1)])), Term::boolean(true));
    assert_eq!(
        evaluate(&Term::apply("And", vec![Term::boolean(true), Term::boolean(false)])),
        Term::boolean(false)
    );
    assert_eq!(evaluate(&Term::apply("Not", vec![Term::boolean(true)])), Term::boolean(false));
}

#[test]
fn part_zero_returns_list_head() {
    let e = evaluate(&Term::apply("Part", vec![Term::List(vec![Term::int(1), Term::int(2), Term::int(3)]), Term::int(0)]));
    assert_eq!(e, Term::symbol("List"));
}

#[test]
fn part_oob_is_invalid_index() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::{ComputationStatus, DiagnosticCode};

    let o = evaluate_outcome(&Term::apply("Part", vec![Term::List(vec![Term::int(1), Term::int(2)]), Term::int(9)]));
    assert!(o.has_error());
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::InvalidIndex);
    assert!(evaluate_checked(&Term::apply("Part", vec![Term::List(vec![Term::int(1)]), Term::int(3)])).is_err());
}

#[test]
fn unsupported_import_is_not_silent_value() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::{ComputationStatus, DiagnosticCode};

    let o = evaluate_outcome(&Term::apply("Import", vec![Term::Atom(athena_engine::Atom::String("x.csv".into()))]));
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::UnsupportedOperation);
}

#[test]
fn unknown_head_is_unevaluated_not_exact_value() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::ComputationStatus;

    let o = evaluate_outcome(&Term::apply("FooBar", vec![Term::int(1)]));
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Unknown);
    assert!(!o.has_error());
}

#[test]
fn as_boolean_accepts_true_false_and_bits() {
    use athena_engine::as_boolean;

    assert_eq!(as_boolean(&Term::boolean(true)), Some(true));
    assert_eq!(as_boolean(&Term::boolean(false)), Some(false));
    assert_eq!(as_boolean(&Term::symbol("True")), Some(true));
    assert_eq!(as_boolean(&Term::symbol("False")), Some(false));
    assert_eq!(as_boolean(&Term::int(1)), Some(true));
    assert_eq!(as_boolean(&Term::int(0)), Some(false));
    assert_eq!(as_boolean(&Term::int(2)), None);
    assert_eq!(as_boolean(&Term::symbol("x")), None);
    assert_eq!(as_boolean(&Term::null()), None);
}

#[test]
fn if_true_branch_and_short_circuit() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::DiagnosticCode;

    let e = evaluate(&Term::apply(
        "If",
        vec![Term::apply("Equal", vec![Term::int(1), Term::int(1)]), Term::int(7), Term::int(8)],
    ));
    assert_eq!(e, Term::int(7));

    // False branch must not evaluate Import (would be UnsupportedOperation).
    let o = evaluate_outcome(&Term::apply(
        "If",
        vec![
            Term::symbol("True"),
            Term::int(7),
            Term::apply("Import", vec![Term::Atom(athena_engine::Atom::String("x.csv".into()))]),
        ],
    ));
    assert_eq!(o.term, Term::int(7));
    assert_eq!(o.kind, EvalKind::Value);
    assert!(!o.diagnostics.iter().any(|d| d.code == DiagnosticCode::UnsupportedOperation));
}

#[test]
fn if_false_and_null_and_non_boolean() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::{ComputationStatus, DiagnosticCode};

    assert_eq!(
        evaluate(&Term::apply("If", vec![Term::symbol("False"), Term::int(7), Term::int(8)])),
        Term::int(8)
    );
    assert_eq!(evaluate(&Term::apply("If", vec![Term::int(0), Term::int(7)])), Term::null());

    let o = evaluate_outcome(&Term::apply("If", vec![Term::symbol("x"), Term::int(1), Term::int(2)]));
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::NonBooleanCondition);
}

#[test]
fn symbol_true_false_null_canonicalize_to_typed_atoms() {
    assert_eq!(evaluate(&Term::symbol("True")), Term::boolean(true));
    assert_eq!(evaluate(&Term::symbol("False")), Term::boolean(false));
    assert_eq!(evaluate(&Term::symbol("Null")), Term::null());
    assert_eq!(
        evaluate(&Term::apply("Equal", vec![Term::int(1), Term::int(1)])),
        Term::boolean(true)
    );
}

#[test]
fn hold_and_hold_form_do_not_eval_args() {
    assert_eq!(
        evaluate(&Term::apply("Hold", vec![Term::apply("Plus", vec![Term::int(1), Term::int(1)])])),
        Term::apply("Hold", vec![Term::apply("Plus", vec![Term::int(1), Term::int(1)])])
    );
    assert_eq!(
        evaluate(&Term::apply("HoldForm", vec![Term::apply("Plus", vec![Term::int(2), Term::int(3)])])),
        Term::apply("HoldForm", vec![Term::apply("Plus", vec![Term::int(2), Term::int(3)])])
    );
}

#[test]
fn which_picks_first_true_branch() {
    let e = evaluate(&Term::apply(
        "Which",
        vec![Term::symbol("False"), Term::int(1), Term::symbol("True"), Term::int(2), Term::symbol("True"), Term::int(3)],
    ));
    assert_eq!(e, Term::int(2));
}

#[test]
fn span_expands_to_list() {
    assert_eq!(
        evaluate(&Term::apply("Span", vec![Term::int(1), Term::int(3)])),
        Term::List(vec![Term::int(1), Term::int(2), Term::int(3)])
    );
    assert_eq!(
        evaluate(&Term::apply("Span", vec![Term::int(1), Term::int(2), Term::int(10)])),
        Term::List(vec![Term::int(1), Term::int(3), Term::int(5), Term::int(7), Term::int(9)])
    );
}

#[test]
fn part_span_slice() {
    let e = evaluate(&Term::apply(
        "Part",
        vec![
            Term::List(vec![Term::int(1), Term::int(2), Term::int(3)]),
            Term::apply("Span", vec![Term::int(1), Term::int(2)]),
        ],
    ));
    assert_eq!(e, Term::List(vec![Term::int(1), Term::int(2)]));
}

#[test]
fn while_false_skips_body() {
    let e = evaluate(&Term::apply("While", vec![Term::int(0), Term::int(1)]));
    assert_eq!(e, Term::null());
}

#[test]
fn compound_set_binds_for_later_stmts() {
    let e = evaluate(&Term::apply(
        "CompoundExpression",
        vec![
            Term::apply("Set", vec![Term::symbol("x"), Term::int(5)]),
            Term::apply("Plus", vec![Term::symbol("x"), Term::int(1)]),
        ],
    ));
    assert_eq!(e, Term::int(6));
}

#[test]
fn part_end_is_last_element() {
    let e = evaluate(&Term::apply(
        "Part",
        vec![Term::List(vec![Term::int(1), Term::int(2), Term::int(3)]), Term::symbol("End")],
    ));
    assert_eq!(e, Term::int(3));
}

#[test]
fn part_all_returns_list() {
    let e = evaluate(&Term::apply(
        "Part",
        vec![Term::List(vec![Term::int(1), Term::int(2)]), Term::symbol("All")],
    ));
    assert_eq!(e, Term::List(vec![Term::int(1), Term::int(2)]));
}

#[test]
fn for_span_last_value() {
    let e = evaluate(&Term::apply(
        "For",
        vec![
            Term::symbol("i"),
            Term::apply("Span", vec![Term::int(1), Term::int(3)]),
            Term::symbol("i"),
        ],
    ));
    assert_eq!(e, Term::int(3));
}

#[test]
fn with_module_block_local_bindings() {
    use athena_engine::clone_term;

    let locals = Term::List(vec![Term::apply("Set", vec![Term::symbol("x"), Term::int(1)])]);
    let body = Term::apply("Plus", vec![Term::symbol("x"), Term::int(1)]);
    assert_eq!(evaluate(&Term::apply("With", vec![clone_term(&locals), clone_term(&body)])), Term::int(2));
    assert_eq!(evaluate(&Term::apply("Module", vec![clone_term(&locals), clone_term(&body)])), Term::int(2));
    assert_eq!(evaluate(&Term::apply("Block", vec![clone_term(&locals), clone_term(&body)])), Term::int(2));
}

#[test]
fn part_column_all_then_index() {
    // MATLAB A(:,2) as Part[matrix, All, 2]
    let matrix = Term::List(vec![
        Term::List(vec![Term::int(1), Term::int(2)]),
        Term::List(vec![Term::int(3), Term::int(4)]),
    ]);
    let e = evaluate(&Term::apply("Part", vec![matrix, Term::symbol("All"), Term::int(2)]));
    assert_eq!(e, Term::List(vec![Term::int(2), Term::int(4)]));
}

#[test]
fn session_set_persists_across_evaluate() {
    use athena_engine::Session;

    let mut session = Session::new();
    assert_eq!(
        session.evaluate(&Term::apply("Set", vec![Term::symbol("x"), Term::int(5)])),
        Term::int(5)
    );
    assert_eq!(
        session.evaluate(&Term::apply("Plus", vec![Term::symbol("x"), Term::int(1)])),
        Term::int(6)
    );
    session.clear_definitions();
    let cleared = session.evaluate(&Term::apply("Plus", vec![Term::symbol("x"), Term::int(1)]));
    assert!(
        matches!(&cleared, Term::Application { head, arguments: args }
            if head.is_symbol("Plus") && args.iter().any(|a| matches!(a, Term::Atom(Atom::Symbol(s)) if s == "x"))),
        "expected free x after clear, got {cleared:?}"
    );
}

#[test]
fn session_compound_set_writes_definitions() {
    use athena_engine::Session;

    let mut session = Session::new();
    let compound = Term::apply(
        "CompoundExpression",
        vec![
            Term::apply("Set", vec![Term::symbol("y"), Term::int(3)]),
            Term::apply("Plus", vec![Term::symbol("y"), Term::int(4)]),
        ],
    );
    assert_eq!(session.evaluate(&compound), Term::int(7));
    assert_eq!(session.evaluate(&Term::symbol("y")), Term::int(3));
}

#[test]
fn session_setdelayed_evaluates_on_use() {
    use athena_engine::Session;

    let mut session = Session::new();
    let delayed = Term::apply(
        "SetDelayed",
        vec![Term::symbol("a"), Term::apply("Plus", vec![Term::int(1), Term::int(1)])],
    );
    assert_eq!(session.evaluate(&delayed), Term::null());
    assert_eq!(session.evaluate(&Term::symbol("a")), Term::int(2));
}

#[test]
fn module_bare_local_is_renamed_unique() {
    let e1 = evaluate(&Term::apply("Module", vec![Term::List(vec![Term::symbol("x")]), Term::symbol("x")]));
    let e2 = evaluate(&Term::apply("Module", vec![Term::List(vec![Term::symbol("x")]), Term::symbol("x")]));
    match (&e1, &e2) {
        (Term::Atom(Atom::Symbol(a)), Term::Atom(Atom::Symbol(b))) => {
            assert!(a.starts_with("x$"), "got {a}");
            assert!(b.starts_with("x$"), "got {b}");
            assert_ne!(a, b);
        }
        other => panic!("expected unique Module symbols, got {other:?}"),
    }
}

#[test]
fn module_local_does_not_clobber_session() {
    use athena_engine::Session;

    let mut session = Session::new();
    session.evaluate(&Term::apply("Set", vec![Term::symbol("x"), Term::int(5)]));
    assert_eq!(
        session.evaluate(&Term::apply(
            "Module",
            vec![
                Term::List(vec![Term::apply("Set", vec![Term::symbol("x"), Term::int(1)])]),
                Term::apply("Plus", vec![Term::symbol("x"), Term::int(1)]),
            ],
        )),
        Term::int(2)
    );
    assert_eq!(session.evaluate(&Term::symbol("x")), Term::int(5));
}

#[test]
fn mldivide_symbolic_stays_unevaluated() {
    use athena_engine::{EvalKind, evaluate_outcome};

    let o = evaluate_outcome(&Term::apply("Mldivide", vec![Term::symbol("A"), Term::symbol("b")]));
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert!(o.term.head_name() == Some("Mldivide"));
}

#[test]
fn mldivide_2x2_exact_unique() {
    // [1,2;3,4] \ [5;6] → [-4; 9/2]
    let a = Term::List(vec![
        Term::List(vec![Term::int(1), Term::int(2)]),
        Term::List(vec![Term::int(3), Term::int(4)]),
    ]);
    let b = Term::List(vec![Term::List(vec![Term::int(5)]), Term::List(vec![Term::int(6)])]);
    let e = evaluate(&Term::apply("Mldivide", vec![a, b]));
    let expected = Term::List(vec![
        Term::List(vec![Term::int(-4)]),
        Term::List(vec![Term::rational_i64(9, 2).unwrap()]),
    ]);
    assert_eq!(e, expected);
}

#[test]
fn pure_function_slot_application() {
    // Function[Power[Slot[1], 2]][4] → 16
    let f = Term::apply(
        "Function",
        vec![Term::apply("Power", vec![Term::apply("Slot", vec![Term::int(1)]), Term::int(2)])],
    );
    let e = evaluate(&Term::Application { head: Box::new(f), arguments: vec![Term::int(4)] });
    assert_eq!(e, Term::int(16));
}

#[test]
fn named_function_application() {
    // Function[x, x^2][3] → 9
    let f = Term::apply(
        "Function",
        vec![Term::symbol("x"), Term::apply("Power", vec![Term::symbol("x"), Term::int(2)])],
    );
    let e = evaluate(&Term::Application { head: Box::new(f), arguments: vec![Term::int(3)] });
    assert_eq!(e, Term::int(9));
}

#[test]
fn map_pure_function_squares() {
    let f = Term::apply(
        "Function",
        vec![Term::apply("Power", vec![Term::apply("Slot", vec![Term::int(1)]), Term::int(2)])],
    );
    let e = evaluate(&Term::apply(
        "Map",
        vec![f, Term::List(vec![Term::int(1), Term::int(2), Term::int(3)])],
    ));
    assert_eq!(e, Term::List(vec![Term::int(1), Term::int(4), Term::int(9)]));
}

#[test]
fn match_q_blank_and_typed_blank() {
    assert_eq!(
        evaluate(&Term::apply("MatchQ", vec![Term::int(1), Term::apply("Blank", vec![])])),
        Term::boolean(true)
    );
    assert_eq!(
        evaluate(&Term::apply(
            "MatchQ",
            vec![Term::int(1), Term::apply("Blank", vec![Term::symbol("Integer")])]
        )),
        Term::boolean(true)
    );
    assert_eq!(
        evaluate(&Term::apply(
            "MatchQ",
            vec![Term::symbol("a"), Term::apply("Blank", vec![Term::symbol("Integer")])]
        )),
        Term::boolean(false)
    );
}

#[test]
fn cases_filters_integers() {
    let e = evaluate(&Term::apply(
        "Cases",
        vec![
            Term::List(vec![Term::int(1), Term::symbol("a"), Term::int(2)]),
            Term::apply("Blank", vec![Term::symbol("Integer")]),
        ],
    ));
    assert_eq!(e, Term::List(vec![Term::int(1), Term::int(2)]));
}

#[test]
fn range_basic() {
    assert_eq!(evaluate(&Term::apply("Range", vec![Term::int(3)])), Term::List(vec![Term::int(1), Term::int(2), Term::int(3)]));
    assert_eq!(
        evaluate(&Term::apply("Range", vec![Term::int(2), Term::int(5)])),
        Term::List(vec![Term::int(2), Term::int(3), Term::int(4), Term::int(5)])
    );
    assert_eq!(
        evaluate(&Term::apply("Range", vec![Term::int(1), Term::int(7), Term::int(2)])),
        Term::List(vec![Term::int(1), Term::int(3), Term::int(5), Term::int(7)])
    );
    assert_eq!(evaluate(&Term::apply("Range", vec![Term::int(0)])), Term::List(vec![]));
}

#[test]
fn table_basic_iterator() {
    let e = evaluate(&Term::apply(
        "Table",
        vec![Term::symbol("i"), Term::List(vec![Term::symbol("i"), Term::int(3)])],
    ));
    assert_eq!(e, Term::List(vec![Term::int(1), Term::int(2), Term::int(3)]));
}

#[test]
fn table_does_not_leak_iterator_binding() {
    let e = evaluate(&Term::apply(
        "CompoundExpression",
        vec![
            Term::apply(
                "Table",
                vec![
                    Term::apply("Plus", vec![Term::symbol("i"), Term::int(1)]),
                    Term::List(vec![Term::symbol("i"), Term::int(2)]),
                ],
            ),
            Term::symbol("i"),
        ],
    ));
    assert_eq!(e, Term::symbol("i"));
}

#[test]
fn apply_plus_list() {
    let e = evaluate(&Term::apply(
        "Apply",
        vec![Term::symbol("Plus"), Term::List(vec![Term::int(1), Term::int(2), Term::int(3)])],
    ));
    assert_eq!(e, Term::int(6));
}

#[test]
fn length_join_first() {
    assert_eq!(
        evaluate(&Term::apply("Length", vec![Term::List(vec![Term::int(1), Term::int(2), Term::int(3)])])),
        Term::int(3)
    );
    assert_eq!(
        evaluate(&Term::apply(
            "Join",
            vec![Term::List(vec![Term::int(1)]), Term::List(vec![Term::int(2)])]
        )),
        Term::List(vec![Term::int(1), Term::int(2)])
    );
    assert_eq!(
        evaluate(&Term::apply("First", vec![Term::List(vec![Term::symbol("a"), Term::symbol("b")])])),
        Term::symbol("a")
    );
}

#[test]
fn sum_and_product_basic() {
    assert_eq!(
        evaluate(&Term::apply(
            "Sum",
            vec![Term::symbol("i"), Term::List(vec![Term::symbol("i"), Term::int(1), Term::int(10)])]
        )),
        Term::int(55)
    );
    assert_eq!(
        evaluate(&Term::apply(
            "Product",
            vec![Term::symbol("i"), Term::List(vec![Term::symbol("i"), Term::int(1), Term::int(5)])]
        )),
        Term::int(120)
    );
}

#[test]
fn matrix_constructors_and_size() {
    assert_eq!(
        evaluate(&Term::apply("Eye", vec![Term::int(2)])),
        Term::List(vec![
            Term::List(vec![Term::int(1), Term::int(0)]),
            Term::List(vec![Term::int(0), Term::int(1)]),
        ])
    );
    assert_eq!(
        evaluate(&Term::apply("Zeros", vec![Term::int(2), Term::int(3)])),
        Term::List(vec![
            Term::List(vec![Term::int(0), Term::int(0), Term::int(0)]),
            Term::List(vec![Term::int(0), Term::int(0), Term::int(0)]),
        ])
    );
    assert_eq!(
        evaluate(&Term::apply("Ones", vec![Term::int(2)])),
        Term::List(vec![
            Term::List(vec![Term::int(1), Term::int(1)]),
            Term::List(vec![Term::int(1), Term::int(1)]),
        ])
    );
    let m = Term::List(vec![
        Term::List(vec![Term::int(1), Term::int(2)]),
        Term::List(vec![Term::int(3), Term::int(4)]),
    ]);
    assert_eq!(
        evaluate(&Term::apply("Size", vec![m])),
        Term::List(vec![Term::int(2), Term::int(2)])
    );
    assert_eq!(
        evaluate(&Term::apply("Size", vec![Term::List(vec![Term::int(1), Term::int(2), Term::int(3)])])),
        Term::List(vec![Term::int(1), Term::int(3)])
    );
}

#[test]
fn elementwise_dot_ops_on_lists() {
    assert_eq!(
        evaluate(&Term::apply(
            "DotTimes",
            vec![Term::List(vec![Term::int(1), Term::int(2)]), Term::List(vec![Term::int(3), Term::int(4)])]
        )),
        Term::List(vec![Term::int(3), Term::int(8)])
    );
    assert_eq!(
        evaluate(&Term::apply("DotTimes", vec![Term::int(2), Term::List(vec![Term::int(1), Term::int(2)])])),
        Term::List(vec![Term::int(2), Term::int(4)])
    );
    assert_eq!(
        evaluate(&Term::apply(
            "DotPower",
            vec![Term::List(vec![Term::int(1), Term::int(2), Term::int(3)]), Term::int(0)]
        )),
        Term::List(vec![Term::int(1), Term::int(1), Term::int(1)])
    );
    assert_eq!(
        evaluate(&Term::apply(
            "DotPower",
            vec![Term::List(vec![Term::int(1), Term::int(2)]), Term::List(vec![Term::int(2), Term::int(3)])]
        )),
        Term::List(vec![Term::int(1), Term::int(8)])
    );
    // Nested matrices
    let a = Term::List(vec![
        Term::List(vec![Term::int(1), Term::int(2)]),
        Term::List(vec![Term::int(3), Term::int(4)]),
    ]);
    let b = Term::List(vec![
        Term::List(vec![Term::int(5), Term::int(6)]),
        Term::List(vec![Term::int(7), Term::int(8)]),
    ]);
    assert_eq!(
        evaluate(&Term::apply("DotTimes", vec![clone_term(&a), clone_term(&b)])),
        Term::List(vec![
            Term::List(vec![Term::int(5), Term::int(12)]),
            Term::List(vec![Term::int(21), Term::int(32)]),
        ])
    );
}

#[test]
fn matrix_det_sum_matmul_linear_solve() {
    let m = Term::List(vec![
        Term::List(vec![Term::int(1), Term::int(2)]),
        Term::List(vec![Term::int(3), Term::int(4)]),
    ]);
    assert_eq!(evaluate(&Term::apply("Det", vec![clone_term(&m)])), Term::int(-2));
    assert_eq!(
        evaluate(&Term::apply("Sum", vec![Term::List(vec![Term::int(1), Term::int(2), Term::int(3)])])),
        Term::int(6)
    );
    assert_eq!(
        evaluate(&Term::apply("Sum", vec![clone_term(&m)])),
        Term::List(vec![Term::int(4), Term::int(6)])
    );
    let b = Term::List(vec![
        Term::List(vec![Term::int(5), Term::int(6)]),
        Term::List(vec![Term::int(7), Term::int(8)]),
    ]);
    assert_eq!(
        evaluate(&Term::apply("Times", vec![clone_term(&m), clone_term(&b)])),
        Term::List(vec![
            Term::List(vec![Term::int(19), Term::int(22)]),
            Term::List(vec![Term::int(43), Term::int(50)]),
        ])
    );
    let rhs = Term::List(vec![Term::List(vec![Term::int(5)]), Term::List(vec![Term::int(6)])]);
    assert_eq!(
        evaluate(&Term::apply("LinearSolve", vec![clone_term(&m), rhs])),
        Term::List(vec![
            Term::List(vec![Term::int(-4)]),
            Term::List(vec![Term::rational_i64(9, 2).unwrap()]),
        ])
    );
    // Symbolic Sum iterator still works
    assert_eq!(
        evaluate(&Term::apply(
            "Sum",
            vec![Term::symbol("i"), Term::List(vec![Term::symbol("i"), Term::int(1), Term::int(10)])]
        )),
        Term::int(55)
    );
}
