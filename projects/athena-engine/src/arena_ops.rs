//! Arena 读写辅助（求值与 lowering 共用）。

use athena_ir::{AtomKind, TermBuilder, TermKind};
use athena_numeric::Number;
use athena_types::{ExprId, SourceSpan, TermId, ValueId};

use crate::session::Session;

/// 默认空 span。
pub fn default_span() -> SourceSpan {
    SourceSpan { start: 0, end: 0 }
}

/// 将 arena 节点注册为表达式身份。
pub fn intern_expr(session: &mut Session, term: TermId) -> ExprId {
    session.exprs.intern_term(term)
}

/// 将 arena 节点注册为值身份。
pub fn intern_value(session: &mut Session, term: TermId) -> ValueId {
    session.value_bindings.intern_term(term)
}

/// 表达式 → 存储项。
pub fn term_of_expr(session: &Session, expr: ExprId) -> Option<TermId> {
    session.exprs.term_of(expr)
}

/// 值 → 存储项。
pub fn term_of_value(session: &Session, value: ValueId) -> Option<TermId> {
    session.value_bindings.term_of(value)
}

/// 构造小型整数节点。
pub fn push_int(session: &mut Session, n: i64) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.int(n, default_span())
}

/// 构造 Boolean 节点。
pub fn push_bool(session: &mut Session, value: bool) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.boolean(value, default_span())
}

/// 构造 Null 节点。
pub fn push_null(session: &mut Session) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.null(default_span())
}

/// 构造符号节点（intern 名称）。
pub fn push_symbol_name(session: &mut Session, name: &str) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.symbol(name, default_span())
}

/// 构造命名应用节点。
pub fn push_app_named(session: &mut Session, head: &str, args: Vec<TermId>) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.app_named(&mut session.operators, head, args, default_span())
}

/// 构造列表节点。
pub fn push_list(session: &mut Session, items: Vec<TermId>) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.list(items, default_span())
}

/// 读取节点种类。
pub fn get_kind<'a>(session: &'a Session, id: TermId) -> Option<&'a TermKind> {
    session.arena.get(id)
}

/// 应用节点的算子名。
pub fn app_head_name(session: &Session, id: TermId) -> Option<String> {
    let TermKind::App { op, .. } = session.arena.get(id)?
    else {
        return None;
    };
    session.operators.name(*op).map(str::to_string)
}

/// 应用节点的参数（拷贝 id 列表）。
pub fn app_args(session: &Session, id: TermId) -> Option<Vec<TermId>> {
    match session.arena.get(id)? {
        TermKind::App { args, .. } => Some(args.clone()),
        _ => None,
    }
}

/// 符号节点的名称。
pub fn symbol_name(session: &Session, id: TermId) -> Option<String> {
    let TermKind::Atom(AtomKind::Symbol(sym)) = session.arena.get(id)?
    else {
        return None;
    };
    session.arena.symbols().resolve(*sym).map(str::to_string)
}

/// 数字载荷引用。
pub fn number_from_id<'a>(session: &'a Session, id: TermId) -> Option<&'a Number> {
    match session.arena.get(id)? {
        TermKind::Atom(AtomKind::Number(n)) => Some(n),
        _ => None,
    }
}

/// 将节点解释为 typed Boolean。
pub fn as_boolean_id(session: &Session, id: TermId) -> Option<bool> {
    match session.arena.get(id)? {
        TermKind::Atom(AtomKind::Boolean(b)) => Some(*b),
        TermKind::Atom(AtomKind::Symbol(sym)) => {
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
pub fn copy_term_subtree(session: &mut Session, id: TermId) -> athena_types::Result<TermId> {
    let ctx = session.numeric_context();
    copy_term_subtree_inner(session, id, &ctx)
}

fn copy_term_subtree_inner(
    session: &mut Session,
    id: TermId,
    ctx: &athena_numeric::NumericContext,
) -> athena_types::Result<TermId> {
    let kind =
        session.arena.get(id).ok_or_else(|| athena_types::Diagnostic::new(athena_types::DiagnosticCode::InvalidIndex))?;
    let cloned = kind.try_clone_in(ctx)?;
    let span = session.arena.span(id).unwrap_or_default();
    Ok(session.arena.push(cloned, span))
}
