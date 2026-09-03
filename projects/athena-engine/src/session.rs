//! Session 与求值环境。

use std::{cell::RefCell, ptr::NonNull, rc::Rc};

use athena_gc::{CollectReport, GcHeap, GcMode, GcObjectId, HeapBudget, Result as GcResult, RootKind, RootToken};
use athena_ir::{OperatorRegistry, TermArena, TermBuilder};
use athena_numeric::{ExecutionBudget, NumericContext};
use athena_types::{ExprId, TermId, ValueId};

use crate::{
    eval::{DefinitionMap, EvalOutcome, evaluate_in, evaluate_with_definitions},
    graph_theory::{GraphTheoryRequest, GraphTheoryResult, execute_graph_theory},
    linear_algebra::{LinearAlgebraRequest, LinearAlgebraResult, execute_linear_algebra},
    mgraph::MGraphState,
    polynomial::{PolynomialRequest, PolynomialResult, RingTable, execute_polynomial_mgraph, execute_polynomial_with_rings},
    semantic::{AssumptionScopeTable, ExprBindingTable, ResultIdTable, ValueIdTable},
    term::Term,
    value::ValueBindingTable,
};

/// 可变求值 Session（绑定、选项、环注册表、M-Graph、语义表、runtime heap roots）。
pub struct Session {
    /// Core IR arena（表达式与求值结果存储）。
    pub arena: TermArena,
    /// 内建算子注册表。
    pub operators: OperatorRegistry,
    /// 表达式身份 ↔ 存储 `TermId`。
    pub exprs: ExprBindingTable,
    /// 值身份 ↔ 存储 `TermId`。
    pub value_bindings: ValueBindingTable,
    /// Own / Delayed 符号定义（跨 `evaluate` 持久）。
    pub definitions: DefinitionMap,
    /// 多项式环 intern 表。
    pub rings: RingTable,
    /// M-Graph 状态（多项式缓存 · witness）。
    pub mgraph: MGraphState,
    /// 值对象身份注册表。
    pub values: ValueIdTable,
    /// 结果容器身份。
    pub results: ResultIdTable,
    /// 假设作用域 intern。
    pub assumption_scopes: AssumptionScopeTable,
    /// Session 级 `athena-gc` heap（object / numeric roots 编排）。
    heap: Rc<RefCell<GcHeap>>,
}

impl core::fmt::Debug for Session {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Session")
            .field("arena_len", &self.arena.len())
            .field("operators", &self.operators.len())
            .field("definitions", &self.definitions.keys().collect::<Vec<_>>())
            .field("rings", &self.rings)
            .field("mgraph", &self.mgraph)
            .field("exprs", &self.exprs)
            .field("value_bindings", &self.value_bindings)
            .field("values", &self.values)
            .field("results", &self.results)
            .field("assumption_scopes", &self.assumption_scopes)
            .field("heap_id", &self.heap.borrow().id())
            .finish()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// 空 session（隔离登记 heap · 基准 [`GcMode::Deferred`]）。
    ///
    /// Living 18：session 算术经 [`Self::numeric_context`] 发布。宿主可见便利入口仍用
    /// [`NumericContext::portable_default`]（共享默认 heap · Auto）。
    pub fn new() -> Self {
        let heap = GcHeap::new_shared(HeapBudget::default());
        heap.borrow().gc().set_base_mode(GcMode::Deferred);
        Self {
            arena: TermArena::new(),
            operators: OperatorRegistry::standard(),
            exprs: ExprBindingTable::default(),
            value_bindings: ValueBindingTable::default(),
            definitions: DefinitionMap::new(),
            rings: RingTable::default(),
            mgraph: MGraphState::default(),
            values: ValueIdTable::default(),
            results: ResultIdTable::default(),
            assumption_scopes: AssumptionScopeTable::default(),
            heap,
        }
    }

    /// 获取可变 [`TermBuilder`]。
    pub fn builder(&mut self) -> TermBuilder<'_> {
        TermBuilder::new(&mut self.arena)
    }

    /// 将存储项注册为表达式身份。
    pub fn intern_expr(&mut self, term: TermId) -> ExprId {
        self.exprs.intern_term(term)
    }

    /// 将存储项注册为值身份。
    pub fn intern_value(&mut self, term: TermId) -> ValueId {
        self.value_bindings.intern_term(term)
    }

    /// 表达式对应的存储项。
    pub fn term_of_expr(&self, expr: ExprId) -> Option<TermId> {
        self.exprs.term_of(expr)
    }

    /// 值对应的存储项。
    pub fn term_of_value(&self, value: ValueId) -> Option<TermId> {
        self.value_bindings.term_of(value)
    }

    /// 在本 Session 定义表上求值（顶层 `Set` 持久化）。
    pub fn evaluate(&mut self, expr: &Term) -> Term {
        evaluate_in(&mut self.definitions, expr)
    }

    /// 带状态 / 诊断的 Session 求值。
    pub fn evaluate_outcome(&mut self, expr: &Term) -> EvalOutcome {
        evaluate_with_definitions(&mut self.definitions, expr)
    }

    /// 清除 Own 符号定义（不触及 heap / rings）。
    pub fn clear_definitions(&mut self) {
        self.definitions.clear();
    }

    /// Session runtime heap。
    pub fn heap(&self) -> &Rc<RefCell<GcHeap>> {
        &self.heap
    }

    /// 绑定本 session heap 的 numeric 发布上下文（继承 Deferred 基准 mode）。
    pub fn numeric_context(&self) -> NumericContext {
        NumericContext::with_heap(ExecutionBudget::unlimited(), self.heap.clone())
    }

    /// 登记 object root（通常 [`RootKind::Session`] / [`RootKind::Ir`]）。
    pub fn register_root(&self, object: GcObjectId, kind: RootKind) -> RootToken {
        self.heap.borrow_mut().roots_mut().register(object, kind)
    }

    /// 取消 object root。
    pub fn unregister_root(&self, token: RootToken) -> bool {
        self.heap.borrow_mut().roots_mut().unregister(token)
    }

    /// 登记 numeric payload root。
    pub fn register_numeric_root(&self, payload: NonNull<u8>, kind: RootKind) -> RootToken {
        self.heap.borrow_mut().roots_mut().register_numeric(payload, kind)
    }

    /// 取消 numeric root。
    pub fn unregister_numeric_root(&self, token: RootToken) -> bool {
        self.heap.borrow_mut().roots_mut().unregister_numeric(token)
    }

    /// 在本 session heap 上执行 tracing collect。
    pub fn collect(&self) -> GcResult<CollectReport> {
        self.heap.borrow_mut().collect()
    }

    /// Tracing collect（注入图 [`athena_gc::ObjectGraph`]）。
    pub fn collect_traced(&self, graph: &dyn athena_gc::ObjectGraph) -> GcResult<CollectReport> {
        self.heap.borrow_mut().collect_traced(graph)
    }

    /// 在 session heap 上 `finish()`：分配真实 object id 并挂图根。
    pub fn finish_graph_on_heap<N, E>(
        &self,
        builder: athena_graph::GraphBuilder<N, E>,
    ) -> core::result::Result<athena_graph::PublishedImmutableGraph<N, E>, athena_graph::GraphError> {
        builder.finish_on_heap(&mut self.heap.borrow_mut())
    }

    /// 在 session heap 上发布 immutable snapshot，并将 CSR 写入 GraphIndex segment。
    pub fn finish_csr_on_heap<N, E>(
        &self,
        builder: athena_graph::GraphBuilder<N, E>,
        registry: &mut athena_graph::ChunkRegistry,
        budget: athena_ndarray::MemoryBudget,
    ) -> core::result::Result<(athena_graph::PublishedImmutableGraph<N, E>, athena_graph::CsrOnHeap), athena_graph::GraphError>
    {
        builder.finish_csr_on_heap(&mut self.heap.borrow_mut(), registry, budget)
    }

    /// 在 session 环表上下文中执行多项式域请求。
    pub fn execute_polynomial(&self, request: PolynomialRequest) -> PolynomialResult {
        execute_polynomial_with_rings(request, &self.rings)
    }

    /// 经 M-Graph 缓存与 witness 记录执行多项式请求。
    pub fn execute_polynomial_mgraph(&mut self, request: PolynomialRequest) -> PolynomialResult {
        execute_polynomial_mgraph(request, &self.rings, &mut self.mgraph)
    }

    /// 执行图论域请求（E0：与 [`crate::execute_domain`] 的 `GraphTheory` 分支等价）。
    pub fn execute_graph_theory(&self, request: GraphTheoryRequest) -> GraphTheoryResult {
        execute_graph_theory(request)
    }

    /// 执行线性代数域请求（与 [`crate::execute_domain`] 的 `LinearAlgebra` 分支等价）。
    pub fn execute_linear_algebra(&self, request: LinearAlgebraRequest) -> LinearAlgebraResult {
        execute_linear_algebra(request)
    }
}
