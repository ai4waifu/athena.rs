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

/// 预驻留的残差扩展标识（按 [`ExtensionOperatorId`] 比较，绝不按显示名）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResidualExtensionIds {
    /// 残差不定式标记。
    pub indeterminate: ExtensionOperatorId,
    /// 收敛域谓词中使用的实部残差。
    pub re: ExtensionOperatorId,
    /// 收敛域 / 定义域谓词中使用的属于残差。
    pub element: ExtensionOperatorId,
    /// 单位阶跃 / Heaviside 因果标记。
    pub unit_step: ExtensionOperatorId,
    /// Heaviside 别名（同一语义残差族）。
    pub heaviside_theta: ExtensionOperatorId,
    /// Kronecker δ 残差。
    pub kronecker_delta: ExtensionOperatorId,
    /// 离散 δ 残差。
    pub discrete_delta: ExtensionOperatorId,
}

/// 领域提供者共享的项读/建能力。
pub struct DomainExecutionContext<'a> {
    s: *mut Session,
    ext: ResidualExtensionIds,
    _marker: PhantomData<&'a mut Session>,
}

impl<'a> DomainExecutionContext<'a> {
    /// 在领域调用期间绑定独占的会话借用。
    pub fn new(s: &'a mut Session) -> Self {
        let ext = ResidualExtensionIds {
            indeterminate: s.extensions.intern("Indeterminate"),
            re: s.extensions.intern("Re"),
            element: s.extensions.intern("Element"),
            unit_step: s.extensions.intern("UnitStep"),
            heaviside_theta: s.extensions.intern("HeavisideTheta"),
            kronecker_delta: s.extensions.intern("KroneckerDelta"),
            discrete_delta: s.extensions.intern("DiscreteDelta"),
        };
        Self { s: s as *mut Session, ext, _marker: PhantomData }
    }

    /// 本会话的残差扩展 id 表。
    pub(crate) fn residual_extensions(&self) -> ResidualExtensionIds {
        self.ext
    }

    /// `head` 是否为预驻留的 `Indeterminate` 残差。
    pub(crate) fn is_indeterminate_extension(&self, head: ApplicationHead) -> bool {
        matches!(head, ApplicationHead::Extension(id) if id == self.ext.indeterminate)
    }

    /// `UnitStep` 或 `HeavisideTheta` 残差头。
    pub(crate) fn is_unit_step_extension(&self, head: ApplicationHead) -> bool {
        matches!(
            head,
            ApplicationHead::Extension(id) if id == self.ext.unit_step || id == self.ext.heaviside_theta
        )
    }

    /// `KroneckerDelta` 或 `DiscreteDelta` 残差头。
    pub(crate) fn is_delta_extension(&self, head: ApplicationHead) -> bool {
        matches!(
            head,
            ApplicationHead::Extension(id) if id == self.ext.kronecker_delta || id == self.ext.discrete_delta
        )
    }

    /// `Element` 残差头。
    pub(crate) fn is_element_extension(&self, head: ApplicationHead) -> bool {
        matches!(head, ApplicationHead::Extension(id) if id == self.ext.element)
    }

    /// 驻留扩展算子 id（ODE 因变量头等 · 非核心数学）。
    pub fn intern_extension(&self, name: &str) -> ExtensionOperatorId {
        self.session_mut().extensions.intern(name)
    }

    /// 按 [`ExtensionOperatorId`] 做扩展应用。
    pub(crate) fn apply_extension(&self, id: ExtensionOperatorId, args: Vec<TermId>) -> TermId {
        self.apply_head(ApplicationHead::Extension(id), args)
    }

    #[inline]
    pub(crate) fn session(&self) -> &Session {
        // SAFETY: 生命周期独占性与原先 `CalculusCtx` 不变量一致。
        unsafe { &*self.s }
    }

    #[inline]
    pub(crate) fn session_mut(&self) -> &mut Session {
        // SAFETY: 串行建造 / 折叠使用；无重叠的 `&mut Session`。
        unsafe { &mut *self.s }
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

    /// 堆上数值引用。
    pub(crate) fn number_of(&self, id: TermId) -> Option<&Number> {
        match self.session().arena.get(id) {
            Some(athena_ir::TermNode::Atom(Atom::Number(n))) => Some(n),
            _ => None,
        }
    }

    /// 当原子为精确小整数时取其整数指数。
    pub(crate) fn int_exp(&self, id: TermId) -> Option<i64> {
        self.number_of(id).and_then(|n| n.as_integer_exp())
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
    pub(crate) fn fold_term(&self, id: TermId) -> TermId {
        match execution::execute_ir_request(self.session_mut(), AthenaRequest::Term(id)) {
            Ok(result_id) => self.session().results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(id),
            Err(_) => id,
        }
    }

    /// 数值原子。
    pub(crate) fn num(&self, n: Number) -> TermId {
        execution::push_number(self.session_mut(), n)
    }

    /// 精确小整数。
    pub fn in_(&self, n: i64) -> TermId {
        crate::runtime::values::arena::push_int(self.session_mut(), n)
    }

    /// 机器浮点原子。
    pub(crate) fn real(&self, x: f64) -> TermId {
        execution::push_number(self.session_mut(), Number::machine(x))
    }

    /// 按显示名构造符号原子（用户符号，非算子）。
    pub(crate) fn symbol(&self, name: &str) -> TermId {
        crate::runtime::values::arena::push_symbol_name(self.session_mut(), name)
    }

    /// 闭数学常量原子。
    pub(crate) fn math_constant(&self, value: athena_ir::MathematicalConstant) -> TermId {
        crate::runtime::values::arena::push_constant(self.session_mut(), value)
    }

    /// 驻留用户符号名。
    pub fn intern(&self, name: &str) -> SymbolId {
        self.session_mut().arena.symbols_mut().intern(name)
    }

    /// 将 [`SymbolId`] 解析为显示名（仅用户符号表）。
    pub(crate) fn symbol_resolve(&self, id: SymbolId) -> &str {
        self.session().arena.symbols().resolve(id).unwrap_or("")
    }

    /// 由已有 [`SymbolId`] 构造符号原子。
    pub fn symbol_id(&self, id: SymbolId) -> TermId {
        let span = athena_ir::TermNode::default_span();
        self.session_mut().arena.push(athena_ir::TermNode::Atom(Atom::Symbol(id)), span)
    }

    /// 显式集合种类（绝不静默使用 `"List"` 头）。
    pub(crate) fn collection(&self, kind: CollectionKind, items: Vec<TermId>) -> TermId {
        let span = athena_ir::TermNode::default_span();
        self.session_mut().arena.push(athena_ir::TermNode::Collection { kind, elements: items }, span)
    }

    /// 有序集合便捷构造。
    pub(crate) fn ordered(&self, items: Vec<TermId>) -> TermId {
        self.collection(CollectionKind::OrderedCollection, items)
    }

    /// 重建时保留已有 [`ApplicationHead`]。
    pub(crate) fn apply_head(&self, head: ApplicationHead, args: Vec<TermId>) -> TermId {
        crate::runtime::values::arena::push_application_head(self.session_mut(), head, args)
    }

    /// 核心语义应用。
    pub fn apply_semantic(&self, op: SemanticOperator, args: Vec<TermId>) -> TermId {
        crate::runtime::values::arena::push_semantic(self.session_mut(), op, args)
    }
}
