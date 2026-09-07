//! `ParameterizedTermPlan`：一次 compile，换局部绑定复用。

use athena_engine::{
    execution::{ParameterizedTermPlan, number_of, push_number},
    runtime::{
        Session,
        values::arena::{push_int, push_semantic, push_symbol_name},
    },
};
use athena_ir::SemanticOperator;
use athena_numeric::Number;
use athena_numeric::to_f64_lossy;

#[test]
fn parameterized_plan_reuses_module_across_bindings() {
    let mut session = Session::new();
    let x = push_symbol_name(&mut session, "x");
    let two = push_int(&mut session, 2);
    let expr = push_semantic(&mut session, SemanticOperator::Power, vec![x, two]);
    let plan = ParameterizedTermPlan::compile(&mut session, expr).expect("compile");
    let fp = plan.fingerprint();

    let vs = session.arena.symbols_mut().intern("x");
    let a = push_number(&mut session, Number::machine(3.0));
    let b = push_number(&mut session, Number::machine(4.0));

    let ra = plan.execute_with_locals(&mut session, &[(vs, a)]).expect("eval 3");
    let rb = plan.execute_with_locals(&mut session, &[(vs, b)]).expect("eval 4");
    assert_eq!(plan.fingerprint(), fp);

    let ya = session.results.get(ra).and_then(|r| r.symbolic_term).and_then(|t| number_of(&session, t)).and_then(to_f64_lossy);
    let yb = session.results.get(rb).and_then(|r| r.symbolic_term).and_then(|t| number_of(&session, t)).and_then(to_f64_lossy);
    assert!((ya.unwrap() - 9.0).abs() < 1e-9);
    assert!((yb.unwrap() - 16.0).abs() < 1e-9);
}
