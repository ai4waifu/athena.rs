//! 将模式绑定代入 [`AthenaRequest`]。

use std::collections::HashMap;

use athena_types::{SymbolId, TermId};

use crate::api::request::{AthenaRequest, ControlPlan, DomainGoal, SessionCommand};
use crate::execution::builtins::patterns::substitute_binds;
use crate::runtime::session::Session;

/// 符号绑定替换：递归代入请求内全部 [`TermId`]。
pub(crate) fn substitute_binds_request(
    session: &mut Session,
    request: AthenaRequest,
    binds: &HashMap<SymbolId, TermId>,
) -> AthenaRequest {
    if binds.is_empty() {
        return request;
    }
    match request {
        AthenaRequest::Term(term) => AthenaRequest::Term(substitute_binds(session, term, binds)),
        AthenaRequest::Command(command) => AthenaRequest::Command(substitute_binds_command(session, command, binds)),
        AthenaRequest::Control(plan) => AthenaRequest::Control(substitute_binds_control(session, plan, binds)),
        AthenaRequest::Goal(goal) => AthenaRequest::Goal(substitute_binds_goal(goal)),
    }
}

fn substitute_binds_command(session: &mut Session, command: SessionCommand, binds: &HashMap<SymbolId, TermId>) -> SessionCommand {
    match command {
        SessionCommand::Define { symbol, value, kind, evaluation } => SessionCommand::Define {
            symbol,
            value: substitute_binds(session, value, binds),
            kind,
            evaluation,
        },
        SessionCommand::DefineMatrix { symbol, matrix } => SessionCommand::DefineMatrix { symbol, matrix },
        SessionCommand::RegisterRuleDispatch { table, rule } => SessionCommand::RegisterRuleDispatch { table, rule },
        SessionCommand::ClearDefinition { symbol } => SessionCommand::ClearDefinition { symbol },
    }
}

fn substitute_binds_control(session: &mut Session, plan: ControlPlan, binds: &HashMap<SymbolId, TermId>) -> ControlPlan {
    match plan {
        ControlPlan::Sequence { steps } => ControlPlan::Sequence {
            steps: steps.into_iter().map(|s| substitute_binds_request(session, s, binds)).collect(),
        },
        ControlPlan::Branch { condition, then_branch, else_branch } => ControlPlan::Branch {
            condition: substitute_binds(session, condition, binds),
            then_branch: Box::new(substitute_binds_request(session, *then_branch, binds)),
            else_branch: else_branch.map(|b| Box::new(substitute_binds_request(session, *b, binds))),
        },
        ControlPlan::Cond { arms, otherwise } => ControlPlan::Cond {
            arms: arms
                .into_iter()
                .map(|(cond, branch)| (substitute_binds(session, cond, binds), Box::new(substitute_binds_request(session, *branch, binds))))
                .collect(),
            otherwise: otherwise.map(|b| Box::new(substitute_binds_request(session, *b, binds))),
        },
        ControlPlan::LoopWhile { condition, body } => ControlPlan::LoopWhile {
            condition: substitute_binds(session, condition, binds),
            body: Box::new(substitute_binds_request(session, *body, binds)),
        },
        ControlPlan::CountedLoop { variable, iterator, body } => ControlPlan::CountedLoop {
            variable: substitute_binds(session, variable, binds),
            iterator: substitute_binds(session, iterator, binds),
            body: Box::new(substitute_binds_request(session, *body, binds)),
        },
        ControlPlan::Iterate { binder, range, body, evaluation } => ControlPlan::Iterate {
            binder: substitute_binds(session, binder, binds),
            range: substitute_binds(session, range, binds),
            body: Box::new(substitute_binds_request(session, *body, binds)),
            evaluation,
        },
        ControlPlan::Recover { body, handler } => ControlPlan::Recover {
            body: Box::new(substitute_binds_request(session, *body, binds)),
            handler: Box::new(substitute_binds_request(session, *handler, binds)),
        },
        ControlPlan::Reject => ControlPlan::Reject,
        ControlPlan::LocalScope { body } => ControlPlan::LocalScope {
            body: Box::new(substitute_binds_request(session, *body, binds)),
        },
        ControlPlan::LexicalScope { body } => ControlPlan::LexicalScope {
            body: Box::new(substitute_binds_request(session, *body, binds)),
        },
        ControlPlan::DynamicScope { body } => ControlPlan::DynamicScope {
            body: Box::new(substitute_binds_request(session, *body, binds)),
        },
        ControlPlan::Index { target, axes } => ControlPlan::Index {
            target: substitute_binds(session, target, binds),
            axes,
        },
        ControlPlan::StoreIndex { target, axes, value } => ControlPlan::StoreIndex {
            target: substitute_binds(session, target, binds),
            axes,
            value: substitute_binds(session, value, binds),
        },
        ControlPlan::Match { target, pattern } => ControlPlan::Match {
            target: substitute_binds(session, target, binds),
            pattern,
        },
        ControlPlan::CollectMatches { source, pattern } => ControlPlan::CollectMatches {
            source: substitute_binds(session, source, binds),
            pattern,
        },
        ControlPlan::CollectRejects { source, pattern } => ControlPlan::CollectRejects {
            source: substitute_binds(session, source, binds),
            pattern,
        },
        ControlPlan::EarlyReturn { value } => ControlPlan::EarlyReturn {
            value: substitute_binds(session, value, binds),
        },
    }
}

fn substitute_binds_goal(goal: DomainGoal) -> DomainGoal {
    goal.owning_copy()
}
