//! Session 与求值环境。

use std::{cell::RefCell, ptr::NonNull, rc::Rc};

use athena_gc::{CollectReport, GcHeap, GcMode, GcObjectId, HeapBudget, Result as GcResult, RootKind, RootToken};
use athena_ir::{OperatorRegistry, TermBuilder, TermStore};
use athena_numeric::{ExecutionBudget, NumericContext};
use athena_types::{TermId, ValueId};

use crate::{
    domains::{
        graph_theory::{GraphTheoryRequest, GraphTheoryResult, execute_graph_theory},
        linear_algebra::{LinearAlgebraRequest, LinearAlgebraResult, execute_linear_algebra},
        polynomial::{
            PolynomialRequest, PolynomialResult, RingTable, execute_polynomial_mgraph, execute_polynomial_with_rings,
        },
    },
    execution::{self, environment::DefinitionLayer, vm::UnitCache},
    reasoning::mgraph::MGraphState,
    runtime::{
        results::{ComputationResult, ResultStore},
        semantic::AssumptionScopeTable,
        values::{RuntimeValue, ValueStore},
    },
};

/// 可变求值 Session（绑定、选项、环注册表、M-Graph、语义表、runtime heap roots）。
pub struct Session {
    /// Core IR 符号项存储。
    pub arena: TermStore,
    /// 内建算子注册表。
    pub operators: OperatorRegistry,
    /// Interp 语句定义层（`SymbolId` 键 · Living `25` 终态）。
    pub defs: DefinitionLayer,
    /// KernelIR 编译缓存（canonical hash → `ExecUnit`）。
    pub units: UnitCache,
    /// `Module` 局部唯一化计数器。
    pub module_counter: u64,
    /// 多项式环 intern 表。
    pub rings: RingTable,
    /// M-Graph 状态（多项式缓存 · witness）。
    pub mgraph: MGraphState,
    /// 运行时值存储（`ValueId` → [`RuntimeValue`]）。
    pub values: ValueStore,
    /// 计算结果存储（`ResultId` → [`ComputationResult`]）。
    pub results: ResultStore,
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
            .field("defs", &self.defs)
            .field("rings", &self.rings)
            .field("mgraph", &self.mgraph)
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
            arena: TermStore::new(),
            operators: OperatorRegistry::new(),
            defs: DefinitionLayer::new(),
            units: UnitCache::new(),
            module_counter: 0,
            rings: RingTable::default(),
            mgraph: MGraphState::default(),
            values: ValueStore::default(),
            results: ResultStore::default(),
            assumption_scopes: AssumptionScopeTable::default(),
            heap,
        }
    }

    /// 获取可变 [`TermBuilder`]。
    pub fn builder(&mut self) -> TermBuilder<'_> {
        TermBuilder::new(&mut self.arena)
    }

    /// 将符号项包装为运行时值（`RuntimeValue::SymbolicTerm`，非双射表）。
    pub fn insert_symbolic_value(&mut self, term: TermId) -> ValueId {
        self.values.insert_symbolic_term(term)
    }

    /// 插入任意运行时值。
    pub fn insert_value(&mut self, value: RuntimeValue) -> ValueId {
        self.values.insert(value)
    }

    /// 若该值载荷是符号项，返回其 [`TermId`]。
    pub fn symbolic_term_of_value(&self, value: ValueId) -> Option<TermId> {
        self.values.get(value).and_then(RuntimeValue::as_symbolic_term)
    }

    /// 记录一次可观察计算结果。
    pub fn insert_result(&mut self, result: ComputationResult) -> athena_types::ResultId {
        self.results.insert(result)
    }

    /// 在本 Session 定义表上求值（顶层 `Set` 持久化 · KernelIR + VM · Living `25`）。
    pub fn evaluate(&mut self, expr: TermId) -> execution::Outcome {
        execution::vm::evaluate_session(self, expr)
    }

    /// 清除符号定义（不触及 heap / rings）。
    pub fn clear_definitions(&mut self) {
        self.defs.clear();
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
