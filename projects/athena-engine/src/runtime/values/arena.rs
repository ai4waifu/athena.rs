//! Arena 读写辅助（求值与 lowering 共用）。

use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermBuilder, TermNode};
use athena_numeric::Number;
use athena_types::{SourceSpan, TermId, ValueId};

use crate::runtime::{
    session::Session,
    values::{RuntimeValue, ValueStore},
};

/// 默认空 span。
pub fn default_span() -> SourceSpan {
    SourceSpan { start: 0, end: 0 }
}

/// 将符号项包装为运行时值（非 `TermId`↔`ValueId` 双射）。
pub fn insert_symbolic_value(session: &mut Session, term: TermId) -> ValueId {
    session.insert_symbolic_value(term)
}

/// 若值载荷是符号项，返回其 [`TermId`]。
pub fn symbolic_term_of_value(session: &Session, value: ValueId) -> Option<TermId> {
    session.symbolic_term_of_value(value)
}

/// 直接向 [`ValueStore`] 插入载荷（测试与宿主辅助）。
pub fn insert_runtime_value(store: &mut ValueStore, value: RuntimeValue) -> ValueId {
    store.insert(value)
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

/// Construct a closed mathematical constant atom.
pub fn push_constant(session: &mut Session, value: athena_ir::MathematicalConstant) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.constant(value, default_span())
}

/// Construct a core semantic application.
pub fn push_semantic(session: &mut Session, op: SemanticOperator, args: Vec<TermId>) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.application_semantic(op, args, default_span())
}

/// Construct an extension application from a registered [`athena_types::OperatorId`].
///
/// Never maps a display string onto core [`SemanticOperator`] semantics (Living `27`).
pub fn push_extension(session: &mut Session, op: athena_types::OperatorId, args: Vec<TermId>) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.application_extension_id(op, args, default_span())
}

/// Construct an application from an explicit head.
pub fn push_application_head(session: &mut Session, head: ApplicationHead, args: Vec<TermId>) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.application(head, args, default_span())
}

/// 构造列表节点。
pub fn push_list(session: &mut Session, items: Vec<TermId>) -> TermId {
    let mut b = TermBuilder::new(&mut session.arena);
    b.list(items, default_span())
}

/// 读取节点种类。
pub fn get_kind<'a>(session: &'a Session, id: TermId) -> Option<&'a TermNode> {
    session.arena.get(id)
}

/// Display / diagnostics / render label for an application head.
///
/// **Not for semantic dispatch** (Living `27`). Prefer [`application_head`] +
/// `match` on [`ApplicationHead`] / [`SemanticOperator`] in core paths.
pub fn application_display_name(session: &Session, id: TermId) -> Option<String> {
    let TermNode::Application { head, .. } = session.arena.get(id)?
    else {
        return None;
    };
    match *head {
        ApplicationHead::Semantic(op) => Some(op.debug_label().to_string()),
        ApplicationHead::Extension(op) => session.operators.name(op).map(str::to_string),
    }
}

/// Application head enum if present.
pub fn application_head(session: &Session, id: TermId) -> Option<ApplicationHead> {
    match session.arena.get(id)? {
        TermNode::Application { head, .. } => Some(*head),
        _ => None,
    }
}

/// 应用节点的参数（拷贝 id 列表）。
pub fn application_arguments(session: &Session, id: TermId) -> Option<Vec<TermId>> {
    match session.arena.get(id)? {
        TermNode::Application { arguments: args, .. } => Some(args.clone()),
        _ => None,
    }
}

/// 符号节点的名称。
pub fn symbol_name(session: &Session, id: TermId) -> Option<String> {
    let TermNode::Atom(Atom::Symbol(sym)) = session.arena.get(id)?
    else {
        return None;
    };
    session.arena.symbols().resolve(*sym).map(str::to_string)
}

/// 数字载荷引用。
pub fn number_from_id<'a>(session: &'a Session, id: TermId) -> Option<&'a Number> {
    match session.arena.get(id)? {
        TermNode::Atom(Atom::Number(n)) => Some(n),
        _ => None,
    }
}

/// 将节点解释为 typed Boolean（仅 `Atom::Boolean` 与精确 `0`/`1` 数字）。
///
/// Living `27`：禁止用用户符号显示名 `"True"` / `"False"` 反推布尔语义。
pub fn as_boolean_id(session: &Session, id: TermId) -> Option<bool> {
    match session.arena.get(id)? {
        TermNode::Atom(Atom::Boolean(b)) => Some(*b),
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

fn copy_term_subtree_inner(session: &mut Session, id: TermId, ctx: &athena_numeric::NumericContext) -> athena_types::Result<TermId> {
    let kind = session.arena.get(id).ok_or_else(|| athena_types::Diagnostic::new(athena_types::DiagnosticCode::InvalidIndex))?;
    let cloned = kind.try_clone_in(ctx)?;
    let span = session.arena.span(id).unwrap_or_default();
    Ok(session.arena.push(cloned, span))
}
