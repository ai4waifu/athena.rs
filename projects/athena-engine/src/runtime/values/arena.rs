//! Arena 读写辅助（求值与 lowering 共用）。

use athena_ir::{Atom, ExprBuilder, ExprNode};
use athena_numeric::Number;
use athena_types::{ExprId, SourceSpan, ValueId};

use crate::runtime::session::Session;

/// 默认空 span。
pub fn default_span() -> SourceSpan {
    SourceSpan { start: 0, end: 0 }
}

/// 将 arena 节点注册为值身份。
pub fn intern_value(session: &mut Session, term: ExprId) -> ValueId {
    session.value_bindings.intern_term(term)
}

/// 值 → 存储表达式。
pub fn expression_of_value(session: &Session, value: ValueId) -> Option<ExprId> {
    session.value_bindings.term_of(value)
}

/// 构造小型整数节点。
pub fn push_int(session: &mut Session, n: i64) -> ExprId {
    let mut b = ExprBuilder::new(&mut session.arena);
    b.int(n, default_span())
}

/// 构造 Boolean 节点。
pub fn push_bool(session: &mut Session, value: bool) -> ExprId {
    let mut b = ExprBuilder::new(&mut session.arena);
    b.boolean(value, default_span())
}

/// 构造 Null 节点。
pub fn push_null(session: &mut Session) -> ExprId {
    let mut b = ExprBuilder::new(&mut session.arena);
    b.null(default_span())
}

/// 构造符号节点（intern 名称）。
pub fn push_symbol_name(session: &mut Session, name: &str) -> ExprId {
    let mut b = ExprBuilder::new(&mut session.arena);
    b.symbol(name, default_span())
}

/// 构造命名应用节点。
pub fn push_app_named(session: &mut Session, head: &str, args: Vec<ExprId>) -> ExprId {
    let mut b = ExprBuilder::new(&mut session.arena);
    b.app_named(&mut session.operators, head, args, default_span())
}

/// 构造列表节点。
pub fn push_list(session: &mut Session, items: Vec<ExprId>) -> ExprId {
    let mut b = ExprBuilder::new(&mut session.arena);
    b.list(items, default_span())
}

/// 读取节点种类。
pub fn get_kind<'a>(session: &'a Session, id: ExprId) -> Option<&'a ExprNode> {
    session.arena.get(id)
}

/// 应用节点的算子名。
pub fn app_head_name(session: &Session, id: ExprId) -> Option<String> {
    let ExprNode::App { op, .. } = session.arena.get(id)?
    else {
        return None;
    };
    session.operators.name(*op).map(str::to_string)
}

/// 应用节点的参数（拷贝 id 列表）。
pub fn app_args(session: &Session, id: ExprId) -> Option<Vec<ExprId>> {
    match session.arena.get(id)? {
        ExprNode::App { args, .. } => Some(args.clone()),
        _ => None,
    }
}

/// 符号节点的名称。
pub fn symbol_name(session: &Session, id: ExprId) -> Option<String> {
    let ExprNode::Atom(Atom::Symbol(sym)) = session.arena.get(id)?
    else {
        return None;
    };
    session.arena.symbols().resolve(*sym).map(str::to_string)
}

/// 数字载荷引用。
pub fn number_from_id<'a>(session: &'a Session, id: ExprId) -> Option<&'a Number> {
    match session.arena.get(id)? {
        ExprNode::Atom(Atom::Number(n)) => Some(n),
        _ => None,
    }
}

/// 将节点解释为 typed Boolean。
pub fn as_boolean_id(session: &Session, id: ExprId) -> Option<bool> {
    match session.arena.get(id)? {
        ExprNode::Atom(Atom::Boolean(b)) => Some(*b),
        ExprNode::Atom(Atom::Symbol(sym)) => {
            let name = session.arena.symbols().resolve(*sym)?;
            if name == "True" {
                Some(true)
            }
            else if name == "False" {
                Some(false)
            }
            else {
                None
            }
        }
        _ => number_from_id(session, id).and_then(|n| {
            if n.is_zero() {
                Some(false)
            }
            else if *n == Number::small_int(1) {
                Some(true)
            }
            else {
                None
            }
        }),
    }
}

/// 深拷贝子树到新 arena 节点。
pub fn copy_term_subtree(session: &mut Session, id: ExprId) -> athena_types::Result<ExprId> {
    let ctx = session.numeric_context();
    copy_term_subtree_inner(session, id, &ctx)
}

fn copy_term_subtree_inner(
    session: &mut Session,
    id: ExprId,
    ctx: &athena_numeric::NumericContext,
) -> athena_types::Result<ExprId> {
    let kind =
        session.arena.get(id).ok_or_else(|| athena_types::Diagnostic::new(athena_types::DiagnosticCode::InvalidIndex))?;
    let cloned = kind.try_clone_in(ctx)?;
    let span = session.arena.span(id).unwrap_or_default();
    Ok(session.arena.push(cloned, span))
}
