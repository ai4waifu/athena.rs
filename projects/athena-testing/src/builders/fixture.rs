//! Session-backed fixture for constructing typed Athena contracts.

use athena_engine::{AthenaEngine, Session};
use athena_types::{ResultId, TermId};

use crate::builders::{DomainRequestBuilder, TermBuilder};

/// Owns one [`Session`] for typed contract construction and execution.
///
/// This is **not** `@sxo/harness` and must not grow dialect / parse APIs.
pub struct SessionFixture {
    session: Session,
    engine: AthenaEngine,
}

impl SessionFixture {
    /// Fresh session + engine handle.
    pub fn new() -> Self {
        Self { session: Session::new(), engine: AthenaEngine::new() }
    }

    /// Mutable session borrow.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Shared session borrow.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Typed term constructor (no named heads).
    pub fn terms(&mut self) -> TermBuilder<'_> {
        TermBuilder::new(&mut self.session)
    }

    /// Domain goal helpers.
    pub fn domain(&self) -> DomainRequestBuilder {
        DomainRequestBuilder
    }

    /// Evaluate a term through the engine IR path.
    pub fn evaluate_term(&mut self, term: TermId) -> TermId {
        self.engine.evaluate(&mut self.session, term)
    }

    /// Execute a neutral [`athena_engine::api::AthenaRequest`].
    pub fn execute_request(&mut self, request: athena_engine::api::AthenaRequest) -> athena_types::Result<ResultId> {
        self.engine.execute_request(&mut self.session, request)
    }

    /// Structural equality on the session arena.
    pub fn structural_eq(&self, a: TermId, b: TermId) -> bool {
        self.session.arena.structural_eq(a, b)
    }
}

impl Default for SessionFixture {
    fn default() -> Self {
        Self::new()
    }
}
