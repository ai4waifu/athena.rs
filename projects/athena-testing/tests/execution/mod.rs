//! Execution smoke via neutral term request.

use athena_testing::{SessionFixture, assert_exact_integer, term_request};

#[test]
fn term_request_roundtrip_evaluate() {
    let mut fx = SessionFixture::new();
    let term = {
        let mut t = fx.terms();
        let a = t.integer(4);
        let b = t.integer(5);
        t.multiply([a, b])
    };
    let _req = term_request(term);
    let out = fx.evaluate_term(term);
    assert_exact_integer(fx.session(), out, 20);
}
