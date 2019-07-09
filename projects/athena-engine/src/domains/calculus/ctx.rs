//! 微积分算法的会话上下文 — `Shape` 读取 + builder 构造 + interp 求值（Living `25` L3）。
//!
//! 领域算法不再持有 owning 树：全部读写都落在调用方的 session `TermStore`，
//! 结果以 `TermId` 返回，子树按 arena 地址共享。
//!
//! Builder 方法取 `&self`：`Cc` 已对 `Session` 持有独占生命周期，嵌套
//! `cc.ap(..., vec![cc.in_(…)])` 在 `&mut self` 下会触发 E0499；与 rustc Arena
//! 相同，经裸指针再借用是安全的（`'_a` 保证无别名 `&mut Session`）。

#![allow(unsafe_code)]

use athena_ir::Atom;
use athena_numeric::Number;
use athena_types::{OperatorId, SymbolId, TermId};
use std::marker::PhantomData;

use crate::{
    api::request::AthenaRequest,
    execution,
    execution::shape::Shape,
    runtime::{session::Session, values::numeric_clone::clone_number},
};

/// 微积分算法上下文。
pub struct CalculusCtx<'a> {
    s: *mut Session,
    _marker: PhantomData<&'a mut Session>,
}

impl<'a> CalculusCtx<'a> {
    /// 绑定 session。
    pub fn new(s: &'a mut Session) -> Self {
        Self { s: s as *mut Session, _marker: PhantomData }
    }

    #[inline]
    fn session(&self) -> &Session {
        // SAFETY: `Cc` 的生命周期独占 `Session`；无其它 `&mut Session` 并存。
        unsafe { &*self.s }
    }

    #[inline]
    fn session_mut(&self) -> &mut Session {
        // SAFETY: 同上；builder / eval 路径串行使用，无重入别名。
        unsafe { &mut *self.s }
    }

    // ---- 读取 ----

    /// 廉价结构快照（不复制数字载荷）。
    pub(crate) fn shape(&self, id: TermId) -> Option<Shape> {
        match self.session().arena.get(id)? {
            athena_ir::TermNode::Atom(Atom::Number(_)) => Some(Shape::Number),
            athena_ir::TermNode::Atom(Atom::String(v)) => Some(Shape::String(v.clone())),
            athena_ir::TermNode::Atom(Atom::Symbol(s)) => Some(Shape::Symbol(*s)),
            athena_ir::TermNode::Atom(Atom::Boolean(b)) => Some(Shape::Bool(*b)),
            athena_ir::TermNode::Atom(Atom::Null) => Some(Shape::Null),
            athena_ir::TermNode::List(items) => Some(Shape::List(items.clone())),
            athena_ir::TermNode::Application { head: op, arguments: args } => Some(Shape::Application(*op, args.clone())),
        }
    }

    /// App 的算子名。
    pub(crate) fn op_name(&self, op: OperatorId) -> &str {
        self.session().operators.name(op).unwrap_or("")
    }

    /// App 形态：(head 名, 参数)。
    pub(crate) fn application(&self, id: TermId) -> Option<(String, Vec<TermId>)> {
        match self.shape(id)? {
            Shape::Application(op, args) => Some((self.op_name(op).to_string(), args)),
            _ => None,
        }
    }

    /// head 名（App 走注册表 · List → `List` · 符号 → 自身）。
    pub(crate) fn head_name(&self, id: TermId) -> Option<String> {
        match self.shape(id)? {
            Shape::Application(op, _) => Some(self.op_name(op).to_string()),
            Shape::List(_) => Some("List".into()),
            Shape::Symbol(s) => Some(self.symbol_name(s).to_string()),
            _ => None,
        }
    }

    /// 符号名。
    pub(crate) fn symbol_name(&self, symbol: SymbolId) -> &str {
        self.session().arena.symbols().resolve(symbol).unwrap_or("")
    }

    /// 符号名是否等于给定名。
    pub(crate) fn symbol_is(&self, symbol: SymbolId, name: &str) -> bool {
        self.symbol_name(symbol) == name
    }

    /// arena 数字引用。
    pub(crate) fn number_of(&self, id: TermId) -> Option<&Number> {
        match self.session().arena.get(id) {
            Some(athena_ir::TermNode::Atom(Atom::Number(n))) => Some(n),
            _ => None,
        }
    }

    /// 数字整数值。
    pub(crate) fn int_exp(&self, id: TermId) -> Option<i64> {
        self.number_of(id).and_then(|n| n.as_integer_exp())
    }

    /// 数字 owning 复制（portable 无界 context · 与 legacy 算法一致）。
    pub(crate) fn copy(&self, n: &Number) -> Number {
        clone_number(n)
    }

    /// 结构等价。
    pub(crate) fn eq(&self, a: TermId, b: TermId) -> bool {
        self.session().arena.structural_eq(a, b)
    }

    // ---- interp 求值 ----

    /// 在内建定义下求值到稳定形（唯一 `ExecutionIR` 路径）。
    pub(crate) fn eval(&self, id: TermId) -> TermId {
        match execution::execute_ir_request(self.session_mut(), AthenaRequest::Term(id)) {
            Ok(result_id) => self
                .session()
                .results
                .get(result_id)
                .and_then(|r| r.symbolic_term)
                .unwrap_or(id),
            Err(_) => id,
        }
    }

    // ---- 构造 ----

    /// 数字原子。
    pub(crate) fn num(&self, n: Number) -> TermId {
        execution::push_number(self.session_mut(), n)
    }

    /// 小型精确整数。
    pub(crate) fn in_(&self, n: i64) -> TermId {
        crate::runtime::values::arena::push_int(self.session_mut(), n)
    }

    /// 机器实数。
    pub(crate) fn real(&self, x: f64) -> TermId {
        execution::push_number(self.session_mut(), Number::machine(x))
    }

    /// 符号原子。
    pub(crate) fn symbol(&self, name: &str) -> TermId {
        crate::runtime::values::arena::push_symbol_name(self.session_mut(), name)
    }

    /// 符号 id。
    pub(crate) fn intern(&self, name: &str) -> SymbolId {
        self.session_mut().arena.symbols_mut().intern(name)
    }

    /// 列表。
    pub(crate) fn list(&self, items: Vec<TermId>) -> TermId {
        crate::runtime::values::arena::push_list(self.session_mut(), items)
    }

    /// 算子应用。
    pub(crate) fn apply(&self, head: &str, args: Vec<TermId>) -> TermId {
        crate::runtime::values::arena::push_application_named(self.session_mut(), head, args)
    }
}
