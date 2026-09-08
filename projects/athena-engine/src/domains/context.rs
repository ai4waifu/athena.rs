//! 领域共享的执行能力。
//!
//! 凡需读/建项的领域共用本上下文。**不是**微积分专用迷你求值器：不做字符串 head 应用、
//! 不做扩展显示名分派、不按符号名猜测算子。

#![allow(unsafe_code)]

use athena_ir::{ApplicationHead, Atom, SemanticOperator};
use athena_numeric::Number;
use athena_types::{CollectionKind, ExtensionOperatorId, SymbolId, TermId};
use std::marker::PhantomData;

use crate::{
    api::request::AthenaRequest,
    execution,
    execution::shape::Shape,
    runtime::{session::Session, values::numeric_clone::clone_number},
};

/// 领域提供者共享的项读/建能力。
///
/// 构造时吃掉独占 [`Session`] 借用并收成裸指针，用 [`PhantomData`] 绑定生命周期。
///
/// **合同**：
/// - 原子建项（`in_` / `num` / `apply_*`）可用 `&self`，便于 `apply(vec![in_(1), …])` 嵌套
/// - `fold_term` / `session_mut` **必须** `&mut self`，禁止从共享借用制造第二份 `&mut Session`
/// - `number_of` 返回拥有副本，禁止长寿命 `&Number` 与建项交错
/// - 阶跃 / 成员 / δ / 不定式 / 实部经 [`SemanticOperator`] 识别，禁止表面名 Extension intern
pub struct DomainExecutionContext<'a> {
    session: *mut Session,
    _borrow: PhantomData<&'a mut Session>,
}

impl<'a> DomainExecutionContext<'a> {
    /// 在领域调用期间绑定独占的会话借用。
    pub fn new(session: &'a mut Session) -> Self {
        Self { session: session as *mut Session, _borrow: PhantomData }
    }

    /// 正无穷数学常量原子（非用户符号 `Infinity`）。
    pub(crate) fn is_positive_infinity_term(&self, term: TermId) -> bool {
        matches!(self.shape(term), Some(Shape::Constant(athena_ir::MathematicalConstant::Infinity)))
    }

    /// `Indeterminate` 中性算子头。
    pub(crate) fn is_indeterminate(&self, head: ApplicationHead) -> bool {
        matches!(head, ApplicationHead::Semantic(SemanticOperator::Indeterminate))
    }

    /// 单位阶跃中性算子头。
    pub(crate) fn is_unit_step(&self, head: ApplicationHead) -> bool {
        matches!(head, ApplicationHead::Semantic(SemanticOperator::UnitStep))
    }

    /// Kronecker / 离散 δ 中性算子头。
    pub(crate) fn is_delta(&self, head: ApplicationHead) -> bool {
        matches!(
            head,
            ApplicationHead::Semantic(SemanticOperator::KroneckerDelta | SemanticOperator::DiscreteDelta)
        )
    }

    /// 集合成员关系中性算子头。
    pub(crate) fn is_member_of(&self, head: ApplicationHead) -> bool {
        matches!(head, ApplicationHead::Semantic(SemanticOperator::MemberOf))
    }

    /// 驻留扩展算子 id（ODE 因变量头等 · 非核心数学）。
    pub fn intern_extension(&self, name: &str) -> ExtensionOperatorId {
        self.session_for_build().extensions.intern(name)
    }

    /// 按 [`ExtensionOperatorId`] 做扩展应用。
    pub(crate) fn apply_extension(&self, id: ExtensionOperatorId, args: Vec<TermId>) -> TermId {
        self.apply_head(ApplicationHead::Extension(id), args)
    }

    #[inline]
    pub(crate) fn session(&self) -> &Session {
        // SAFETY: `new` 吃掉的 `&mut Session` 在 `'a` 内唯一；只读借用。
        unsafe { &*self.session }
    }

    /// 需要执行折叠 / 重入 IR 时的独占会话借用（禁止 `&self` 入口）。
    #[inline]
    pub(crate) fn session_mut(&mut self) -> &mut Session {
        // SAFETY: `&mut self` 证明此时无重叠的 `DomainExecutionContext` 读借用跨越本调用。
        unsafe { &mut *self.session }
    }

    /// 短生命周期建项：仅 arena / extension intern，立即返回拥有 `TermId`。
    #[inline]
    fn session_for_build(&self) -> &mut Session {
        // SAFETY: 原子建项不返回指向 Session 内部的引用；调用方不得在持有
        // `number_of` 等内部借用时嵌套调用本路径（`number_of` 已改为拥有副本）。
        unsafe { &mut *self.session }
    }

    /// 廉价结构快照（不克隆数值载荷）。
    pub(crate) fn shape(&self, id: TermId) -> Option<Shape> {
        match self.session().arena.get(id)? {
            athena_ir::TermNode::Atom(Atom::Number(_)) => Some(Shape::Number),
            athena_ir::TermNode::Atom(Atom::String(v)) => Some(Shape::String(v.clone())),
            athena_ir::TermNode::Atom(Atom::Symbol(s)) => Some(Shape::Symbol(*s)),
            athena_ir::TermNode::Atom(Atom::Boolean(b)) => Some(Shape::Bool(*b)),
            athena_ir::TermNode::Atom(Atom::Null) => Some(Shape::Null),
            athena_ir::TermNode::Atom(Atom::Constant(c)) => Some(Shape::Constant(*c)),
            athena_ir::TermNode::Collection { elements: items, .. } => Some(Shape::Collection(items.clone())),
            athena_ir::TermNode::Application { head: op, arguments: args } => Some(Shape::Application(*op, args.clone())),
        }
    }

    /// 带类型的应用头与参数。
    pub(crate) fn application_head(&self, id: TermId) -> Option<(ApplicationHead, Vec<TermId>)> {
        match self.shape(id)? {
            Shape::Application(op, args) => Some((op, args)),
            _ => None,
        }
    }

    /// 堆上数值的拥有副本（禁止返回 `&Number` 以免与建项交错）。
    pub(crate) fn number_of(&self, id: TermId) -> Option<Number> {
        match self.session().arena.get(id) {
            Some(athena_ir::TermNode::Atom(Atom::Number(n))) => Some(clone_number(n)),
            _ => None,
        }
    }

    /// 当原子为精确小整数时取其整数指数。
    pub(crate) fn int_exp(&self, id: TermId) -> Option<i64> {
        self.number_of(id).as_ref().and_then(|n| n.as_integer_exp())
    }

    /// 可移植折叠路径用的拥有式数值副本。
    pub(crate) fn copy(&self, n: &Number) -> Number {
        clone_number(n)
    }

    /// 会话堆上的结构相等。
    pub(crate) fn eq(&self, a: TermId, b: TermId) -> bool {
        self.session().arena.structural_eq(a, b)
    }

    /// 符号原子是否等于给定 [`SymbolId`]。
    pub(crate) fn symbol_id_is(&self, symbol: SymbolId, expected: SymbolId) -> bool {
        symbol == expected
    }

    /// 经唯一的 `ExecutionIR` 路径折叠（显式项请求，绝不用字符串头）。
    ///
    /// 失败诊断向上传播，禁止吞成原项。
    pub(crate) fn fold_term(&mut self, id: TermId) -> athena_types::Result<TermId> {
        let result_id = execution::execute_ir_request(self.session_mut(), AthenaRequest::Term(id))?;
        self.session().results.require_symbolic_term(result_id)
    }

    /// 数值原子。
    pub(crate) fn num(&self, n: Number) -> TermId {
        execution::push_number(self.session_for_build(), n)
    }

    /// 精确小整数。
    pub fn in_(&self, n: i64) -> TermId {
        crate::runtime::values::arena::push_int(self.session_for_build(), n)
    }

    /// 机器浮点原子。
    pub(crate) fn real(&self, x: f64) -> TermId {
        execution::push_number(self.session_for_build(), Number::machine(x))
    }

    /// 按显示名构造符号原子（用户符号，非算子）。
    pub(crate) fn symbol(&self, name: &str) -> TermId {
        crate::runtime::values::arena::push_symbol_name(self.session_for_build(), name)
    }

    /// 闭数学常量原子。
    pub(crate) fn math_constant(&self, value: athena_ir::MathematicalConstant) -> TermId {
        crate::runtime::values::arena::push_constant(self.session_for_build(), value)
    }

    /// 驻留用户符号名。
    pub fn intern(&self, name: &str) -> SymbolId {
        self.session_for_build().arena.symbols_mut().intern(name)
    }

    /// 将 [`SymbolId`] 解析为显示名（仅用户符号表）。
    pub(crate) fn symbol_resolve(&self, id: SymbolId) -> &str {
        self.session().arena.symbols().resolve(id).unwrap_or("")
    }

    /// 由已有 [`SymbolId`] 构造符号原子。
    pub fn symbol_id(&self, id: SymbolId) -> TermId {
        let span = athena_ir::TermNode::default_span();
        self.session_for_build().arena.push(athena_ir::TermNode::Atom(Atom::Symbol(id)), span)
    }

    /// 显式集合种类（绝不静默使用 `"List"` 头）。
    pub(crate) fn collection(&self, kind: CollectionKind, items: Vec<TermId>) -> TermId {
        let span = athena_ir::TermNode::default_span();
        self.session_for_build().arena.push(athena_ir::TermNode::Collection { kind, elements: items }, span)
    }

    /// 有序集合便捷构造。
    pub(crate) fn ordered(&self, items: Vec<TermId>) -> TermId {
        self.collection(CollectionKind::OrderedCollection, items)
    }

    /// 重建时保留已有 [`ApplicationHead`]。
    pub(crate) fn apply_head(&self, head: ApplicationHead, args: Vec<TermId>) -> TermId {
        crate::runtime::values::arena::push_application_head(self.session_for_build(), head, args)
    }

    /// 核心语义应用。
    pub fn apply_semantic(&self, op: SemanticOperator, args: Vec<TermId>) -> TermId {
        crate::runtime::values::arena::push_semantic(self.session_for_build(), op, args)
    }
}
