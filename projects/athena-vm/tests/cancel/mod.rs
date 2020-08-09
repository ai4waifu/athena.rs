//! 取消令牌合同。

use athena_vm::CancellationToken;

#[test]
fn cancel_is_shared() {
    let a = CancellationToken::new();
    let b = a.clone();
    assert!(!a.is_cancelled());
    b.cancel();
    assert!(a.is_cancelled());
    a.reset();
    assert!(!b.is_cancelled());
}
