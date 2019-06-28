//! `ExecutionCompiler` — `AthenaRequest` + Session snapshot → [`ExecutionModule`].
//!
//! Bootstrap lowering covers immutable atom terms only. No bridge to the old
//! stack VM / `KernelIR` path.

use athena_ir::TermNode;
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

use crate::api::request::AthenaRequest;
use crate::execution::ir::{
    BasicBlock, BlockId, CapturedRoot, CapturedRootId, ExecutionModule, ExecutionValueType, ModuleFingerprint, Operation,
    OperationKind, Region, RegionId, SsaValueId, verify_module,
};
use crate::runtime::session::Session;

/// Compiles one request into a verified [`ExecutionModule`].
#[derive(Debug, Default)]
pub struct ExecutionCompiler {}

impl ExecutionCompiler {
    /// Create a compiler instance.
    pub fn new() -> Self {
        Self {}
    }

    /// Lower a request against a Session snapshot into `ExecutionIR`.
    pub fn compile(&self, session: &Session, request: &AthenaRequest) -> Result<ExecutionModule> {
        match request {
            AthenaRequest::Term(term) => self.compile_term(session, *term),
            AthenaRequest::Command(_) | AthenaRequest::Control(_) | AthenaRequest::Goal(_) => {
                Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionCompiler")
                    .detail("status", "request_kind_not_lowered")
                    .detail("kind", request.kind_name()))
            }
        }
    }

    fn compile_term(&self, session: &Session, term: TermId) -> Result<ExecutionModule> {
        let Some(node) = session.arena.get(term) else {
            return Err(Diagnostic::new(DiagnosticCode::InvalidIndex)
                .detail("component", "ExecutionCompiler")
                .detail("reason", "missing_term"));
        };
        match node {
            TermNode::Atom(_) => self.compile_atom_term(term),
            TermNode::List(_) | TermNode::Application { .. } => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionCompiler")
                .detail("status", "compound_term_not_lowered")),
        }
    }

    fn compile_atom_term(&self, term: TermId) -> Result<ExecutionModule> {
        let root_id = CapturedRootId(0);
        let value = SsaValueId(0);
        let block = BasicBlock {
            id: BlockId(0),
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(value),
                result_type: ExecutionValueType::Term,
                kind: OperationKind::LoadTerm { root: root_id },
                effect_in: None,
                effect_out: None,
            }],
            terminator: crate::execution::ir::Terminator::return_value(value),
        };
        let region = Region::from_entry_block(RegionId(0), block, vec![ExecutionValueType::Term]);
        let mut module = ExecutionModule {
            inputs: Vec::new(),
            constants: Vec::new(),
            captured_roots: vec![CapturedRoot::term(term)],
            regions: vec![region],
            effect_edges: Vec::new(),
            exits: Vec::new(),
            provider_calls: Vec::new(),
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        verify_module(&module)?;
        Ok(module)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::session::Session;

    #[test]
    fn compile_atom_term_module() {
        let mut session = Session::new();
        let term = session.builder().int(3, Default::default());
        let module = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect("atom");
        assert_eq!(module.captured_roots, vec![CapturedRoot::term(term)]);
        assert_eq!(module.regions.len(), 1);
    }

    #[test]
    fn compile_application_rejected() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", Default::default());
        let plus = session.operators.intern("Plus");
        let term = session.builder().application(plus, vec![x, x], Default::default());
        let err = ExecutionCompiler::new()
            .compile(&session, &AthenaRequest::Term(term))
            .expect_err("compound");
        assert_eq!(err.code, DiagnosticCode::UnsupportedOperation);
    }
}
