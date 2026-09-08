//! `VmHost` 合同。

use athena_vm::{HostOutcome, NullHost, ProviderOpId, SemanticOpId, VmHost};

#[test]
fn null_host_rejects_semantic_and_provider() {
    let mut host = NullHost;
    let sem = host.apply_semantic(SemanticOpId(1), &[]).expect("ok wrapper");
    match sem {
        HostOutcome::Diagnostic(d) => {
            assert_eq!(d.details.get("reason").map(|v| v.to_string()).as_deref(), Some("apply_semantic_unimplemented"));
        }
        other => panic!("expected diagnostic, got {other:?}"),
    }
    let prov = host.call_provider(ProviderOpId(0), &[]).expect("ok wrapper");
    match prov {
        HostOutcome::Diagnostic(d) => {
            assert_eq!(d.details.get("reason").map(|v| v.to_string()).as_deref(), Some("call_provider_unimplemented"));
        }
        other => panic!("expected diagnostic, got {other:?}"),
    }
}
