//! 类型化相等 / 诊断断言（无渲染字符串神谕）。

use athena_engine::Session;
use athena_numeric::Number;
use athena_types::{DiagnosticCode, TermId};

/// 断言两个 term 在 `session` 中结构相等。
pub fn assert_structural_eq(session: &Session, left: TermId, right: TermId) {
    assert!(session.arena.structural_eq(left, right), "terms not structurally equal: {left:?} vs {right:?}");
}

/// 断言 `term` 为等于 `expected` 的精确整数原子。
pub fn assert_exact_integer(session: &Session, term: TermId, expected: i64) {
    let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) = session.arena.get(term)
    else {
        panic!("expected integer atom, got {term:?}");
    };
    let got = n.as_integer_exp().unwrap_or_else(|| panic!("expected integer, got {n:?}"));
    assert_eq!(got, expected);
    let _ = Number::small_int(expected);
}

/// 断言存在某诊断码。
pub fn expect_diagnostic(codes: &[DiagnosticCode], expected: DiagnosticCode) {
    assert!(codes.iter().any(|c| *c == expected), "missing diagnostic {expected:?} in {codes:?}");
}
