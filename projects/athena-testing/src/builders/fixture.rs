//! 基于 `Session` 的 fixture，用于构造类型化 Athena 合同。

use athena_engine::{AthenaEngine, Session};
use athena_types::{ResultId, TermId};

use crate::builders::{DomainRequestBuilder, TermBuilder};

/// 拥有一个 [`Session`]，用于类型化合同构造与执行。
///
/// 这 **不是** `@sxo/harness`，且不得扩展方言 / 解析 API。
pub struct SessionFixture {
    session: Session,
    engine: AthenaEngine,
}

impl SessionFixture {
    /// 新 `Session` + engine 句柄。
    pub fn new() -> Self {
        Self { session: Session::new(), engine: AthenaEngine::new() }
    }

    /// 可变 `Session` 借用。
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// 共享 `Session` 借用。
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// 类型化 term 构造器（无具名 head）。
    pub fn terms(&mut self) -> TermBuilder<'_> {
        TermBuilder::new(&mut self.session)
    }

    /// 领域目标辅助。
    pub fn domain(&self) -> DomainRequestBuilder {
        DomainRequestBuilder
    }

    /// 经 engine IR 路径求值 term。
    pub fn evaluate_term(&mut self, term: TermId) -> TermId {
        self.engine.evaluate(&mut self.session, term)
    }

    /// 执行中立 [`athena_engine::api::AthenaRequest`]。
    pub fn execute_request(&mut self, request: athena_engine::api::AthenaRequest) -> athena_types::Result<ResultId> {
        self.engine.execute_request(&mut self.session, request)
    }

    /// 在 session arena 上做结构相等。
    pub fn structural_eq(&self, a: TermId, b: TermId) -> bool {
        self.session.arena.structural_eq(a, b)
    }
}

impl Default for SessionFixture {
    fn default() -> Self {
        Self::new()
    }
}
