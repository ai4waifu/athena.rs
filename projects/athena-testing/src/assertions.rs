//! Typed equality / diagnostic assertions (no render-string oracles).

use athena_engine::Session;
use athena_numeric::Number;
use athena_types::{DiagnosticCode, TermId};

/// Assert two terms are structurally equal in `session`.
pub fn assert_structural_eq(session: &Session, left: TermId, right: TermId) {
    assert!(session.arena.structural_eq(left, right), "terms not structurally equal: {left:?} vs {right:?}");
}

/// Assert `term` is an exact integer atom equal to `expected`.
pub fn assert_exact_integer(session: &Session, term: TermId, expected: i64) {
    let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) = session.arena.get(term)
    else {
        panic!("expected integer atom, got {term:?}");
    };
    let got = n.as_integer_exp().unwrap_or_else(|| panic!("expected integer, got {n:?}"));
    assert_eq!(got, expected);
    let _ = Number::small_int(expected);
}

/// Assert a diagnostic code is present.
pub fn expect_diagnostic(codes: &[DiagnosticCode], expected: DiagnosticCode) {
    assert!(codes.iter().any(|c| *c == expected), "missing diagnostic {expected:?} in {codes:?}");
}
