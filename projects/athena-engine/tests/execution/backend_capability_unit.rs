//! `VmCapabilityReport` 单元测试。

use athena_engine::{
    Session,
    api::request::AthenaRequest,
    execution::{
        backend::{BackendKind, analyze_vm_capability, select_execution_backend},
        compiler::ExecutionCompiler,
    },
};
use athena_ir::{ApplicationHead, SemanticOperator};

#[test]
fn and_of_integers_selects_athena_vm_after_host_truthiness() {
    let mut session = Session::new();
    let a = session.builder().int(0, Default::default());
    let b = session.builder().int(1, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::And),
        vec![a, b],
        Default::default(),
    );
    let module = ExecutionCompiler::new()
        .compile(&mut session, &AthenaRequest::Term(term))
        .expect("compile");
    let report = analyze_vm_capability(&module);
    assert!(report.supports_athena_vm, "gaps={:?}", report.gaps);
    assert_eq!(select_execution_backend(&module, false), BackendKind::AthenaVm);
    athena_engine::execution::vm_codegen::validate_vm_codegen_subset(&module).expect("validate");
}

#[test]
fn sum_over_iterator_selects_athena_vm_after_host_fold() {
    let mut session = Session::new();
    let k = session.builder().symbol("k", Default::default());
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let four = session.builder().int(4, Default::default());
    let iter = session
        .builder()
        .collection(athena_types::CollectionKind::OrderedCollection, vec![k, one, four], Default::default());
    let body = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Power),
        vec![k, two],
        Default::default(),
    );
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Sum),
        vec![body, iter],
        Default::default(),
    );
    let module = ExecutionCompiler::new()
        .compile(&mut session, &AthenaRequest::Term(term))
        .expect("compile");
    let report = analyze_vm_capability(&module);
    assert!(report.supports_athena_vm, "gaps={:?}", report.gaps);
    assert_eq!(select_execution_backend(&module, false), BackendKind::AthenaVm);
    athena_engine::execution::vm_codegen::validate_vm_codegen_subset(&module).expect("validate");
}

#[test]
fn boolean_not_still_selects_athena_vm() {
    let mut session = Session::new();
    let t = session.builder().boolean(true, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Not),
        vec![t],
        Default::default(),
    );
    let module = ExecutionCompiler::new()
        .compile(&mut session, &AthenaRequest::Term(term))
        .expect("compile");
    let report = analyze_vm_capability(&module);
    assert!(report.supports_athena_vm, "gaps={:?}", report.gaps);
    assert_eq!(select_execution_backend(&module, false), BackendKind::AthenaVm);
    athena_engine::execution::vm_codegen::validate_vm_codegen_subset(&module).expect("validate");
}

#[test]
fn differentiate_residual_selects_athena_vm() {
    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Differentiate),
        vec![x],
        Default::default(),
    );
    let module = ExecutionCompiler::new()
        .compile(&mut session, &AthenaRequest::Term(term))
        .expect("compile");
    let report = analyze_vm_capability(&module);
    assert!(report.supports_athena_vm, "gaps={:?}", report.gaps);
    assert_eq!(select_execution_backend(&module, false), BackendKind::AthenaVm);
    athena_engine::execution::vm_codegen::validate_vm_codegen_subset(&module).expect("validate");
}

#[test]
fn map_sin_list_selects_athena_vm() {
    let mut session = Session::new();
    let sin = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::from_unary(athena_ir::UnaryFunction::Sin)),
        vec![],
        Default::default(),
    );
    let zero = session.builder().int(0, Default::default());
    let list = session.builder().list(vec![zero], Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Map),
        vec![sin, list],
        Default::default(),
    );
    let module = ExecutionCompiler::new()
        .compile(&mut session, &AthenaRequest::Term(term))
        .expect("compile");
    let report = analyze_vm_capability(&module);
    assert!(report.supports_athena_vm, "gaps={:?}", report.gaps);
    assert_eq!(select_execution_backend(&module, false), BackendKind::AthenaVm);
    athena_engine::execution::vm_codegen::validate_vm_codegen_subset(&module).expect("validate");
}
