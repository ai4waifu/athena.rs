//! 请求构造器。

use athena_engine::api::{AthenaRequest, DomainGoal};
use athena_types::TermId;

/// `AthenaRequest::Term`。
pub fn term_request(term: TermId) -> AthenaRequest {
    AthenaRequest::Term(term)
}

/// `AthenaRequest::Goal`。
pub fn goal_request(goal: DomainGoal) -> AthenaRequest {
    AthenaRequest::Goal(goal)
}
