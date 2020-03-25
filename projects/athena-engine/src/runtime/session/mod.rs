//! Session 与求值环境。

use std::{cell::RefCell, ptr::NonNull, rc::Rc};

use athena_gc::{CollectReport, GcHeap, GcMode, GcObjectId, HeapBudget, Result as GcResult, RootKind, RootToken};
use athena_ir::{ExtensionRegistry, TermBuilder, TermStore};
use athena_numeric::{ExecutionBudget, NumericContext};
use athena_types::{TermId, ValueId};

use athena_rewriter::RuleSet;

use crate::{
    api::request::AthenaRequest,
    domains::{
        algebra::GroupTable,
        calculus::SeriesObjectStore,
        field::{FieldRequest, FieldResult, execute_field_with_table_mut},
        galois::{GaloisRequest, GaloisResult, execute_galois_with_tables},
        graph_theory::{GraphTheoryRequest, GraphTheoryResult, execute_graph_theory},
        group::{GroupRequest, GroupResult, execute_group_with_table_mut},
        linear_algebra::{LinearAlgebraRequest, LinearAlgebraResult, MatrixObjectStore, execute_linear_algebra},
        polynomial::{
            PolynomialObjectStore, PolynomialRequest, PolynomialResult, RingTable, execute_polynomial_mgraph, execute_polynomial_with_rings,
        },
    },
    execution::{
        self,
        environment::{CompiledRuleStore, DefinitionLayer},
    },
    reasoning::{
        egraph::{
            CandidateEquivalence, EGraph, SaturationBudget, SaturationReport, TypedRuleSet, admit_application_congruence,
            admit_application_congruence_candidates, admit_structural_candidates, admit_structural_term_equality,
            admit_typed_rewrite_candidates, application_congruence_candidates, saturate, saturate_typed,
        },
        mgraph::{AdmissionRejectReason, ClosureLimits, ClosureResult, FactId, MGraphState, VerificationPolicy},
    },
    runtime::{
        frontier::FrontierStore,
        results::{ComputationResult, ResultStore},
        semantic::AssumptionScopeTable,
        values::{RuntimeValue, ValueStore},
    },
};

/// 类型化 saturation 之后，再经结构接纳、改写回放接纳与应用同余接纳的报告。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct TypedEgraphAdmitReport {
    /// saturation 产生的未验证候选。
    pub saturation: SaturationReport,
    /// 上述候选的结构接纳结果（改写形态对已在上游跳过）。
    pub structural_admitted: Vec<Result<FactId, AdmissionRejectReason>>,
    /// 类型化改写回放接纳结果（`match_pattern` + `substitute`）。
    pub rewrite_admitted: Vec<Result<FactId, AdmissionRejectReason>>,
    /// ExactUF 应用同余接纳结果。
    pub congruence_admitted: Vec<Result<FactId, AdmissionRejectReason>>,
}

impl TypedEgraphAdmitReport {
    /// Owning 复制（经 [`SaturationReport::owning_copy`]）。
    pub fn owning_copy(&self) -> Self {
        Self {
            saturation: self.saturation.owning_copy(),
            structural_admitted: self.structural_admitted.clone(),
            rewrite_admitted: self.rewrite_admitted.clone(),
            congruence_admitted: self.congruence_admitted.clone(),
        }
    }
}

/// 可变求值 Session（绑定、选项、环注册表、M-Graph、语义表、runtime heap roots）。
pub struct Session {
    /// Core IR 符号项存储。
    pub arena: TermStore,
    /// 扩展显示名注册表（非核心算子 catalog）。
    pub extensions: ExtensionRegistry,
    /// Interp 语句定义层（`SymbolId` 键 · 终态）。
    pub defs: DefinitionLayer,
    /// 已编译规则仓（`CompiledRuleId` · `SessionCommand::RegisterRuleDispatch`）。
    pub compiled_rules: CompiledRuleStore,
    /// `LexicalScope` 局部唯一化计数器。
    pub module_counter: u64,
    /// 多项式环 intern 表。
    pub rings: RingTable,
    /// Session 级群 / presentation 注册表（/ 群论 DomainObject）。
    pub groups: GroupTable,
    /// 多项式 DomainObject 仓（`PolynomialRef` → payload）。
    pub polynomial_objects: PolynomialObjectStore,
    /// 级数 DomainObject 仓（`SeriesRef` → payload）。
    pub series_objects: SeriesObjectStore,
    /// 矩阵 DomainObject 仓（`MatrixRef` → payload · `MatrixId` 语义）。
    pub matrix_objects: MatrixObjectStore,
    /// M-Graph 状态（多项式缓存 · witness）。
    pub mgraph: MGraphState,
    /// Scope-local E-Graph（候选搜索 · 不得绕过 AdmissionGate）。
    pub egraph: EGraph,
    /// Session 默认 E-Graph saturation 预算。
    pub egraph_budget: SaturationBudget,
    /// 运行时值存储（`ValueId` → [`RuntimeValue`]）。
    pub values: ValueStore,
    /// 计算结果存储（`ResultId` → [`ComputationResult`]）。
    pub results: ResultStore,
    /// 可恢复前沿存储（`FrontierId` → [`crate::runtime::ComputationFrontier`] · ）。
    pub frontiers: FrontierStore,
    /// 假设作用域 intern。
    pub assumption_scopes: AssumptionScopeTable,
    /// Session 级 `athena-gc` heap（object / numeric roots 编排）。
    heap: Rc<RefCell<GcHeap>>,
}

impl core::fmt::Debug for Session {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Session")
            .field("arena_len", &self.arena.len())
            .field("extensions", &self.extensions.len())
            .field("defs", &self.defs)
            .field("compiled_rules_len", &self.compiled_rules.len())
            .field("rings", &self.rings)
            .field("groups", &self.groups)
            .field("polynomial_objects_len", &self.polynomial_objects.len())
            .field("series_objects_len", &self.series_objects.len())
            .field("matrix_objects_len", &self.matrix_objects.len())
            .field("mgraph", &self.mgraph)
            .field("egraph_eclasses", &self.egraph.eclass_count())
            .field("egraph_budget", &self.egraph_budget)
            .field("values", &self.values)
            .field("results", &self.results)
            .field("frontiers", &self.frontiers)
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
    /// session 算术经 [`Self::numeric_context`] 发布。宿主可见便利入口仍用
    /// [`NumericContext::portable_default`]（共享默认 heap · Auto）。
    pub fn new() -> Self {
        let heap = GcHeap::new_shared(HeapBudget::default());
        heap.borrow().gc().set_base_mode(GcMode::Deferred);
        Self {
            arena: TermStore::new(),
            extensions: ExtensionRegistry::new(),
            defs: DefinitionLayer::new(),
            compiled_rules: CompiledRuleStore::new(),
            module_counter: 0,
            rings: RingTable::default(),
            groups: GroupTable::new(),
            polynomial_objects: PolynomialObjectStore::new(),
            series_objects: SeriesObjectStore::new(),
            matrix_objects: MatrixObjectStore::new(),
            mgraph: MGraphState::default(),
            egraph: EGraph::new(),
            egraph_budget: SaturationBudget::smoke(),
            values: ValueStore::default(),
            results: ResultStore::default(),
            frontiers: FrontierStore::new(),
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

    /// 记录一次可恢复计算前沿。
    pub fn insert_frontier(&mut self, frontier: crate::runtime::ComputationFrontier) -> athena_types::FrontierId {
        self.frontiers.insert(frontier)
    }

    /// 收集 Session M-Graph 中已接纳关系的证书指纹（供 `ResumeCheck.available_certificates`）。
    pub fn available_certificate_fingerprints(&self) -> Vec<u64> {
        let mut out = Vec::new();
        for record in self.mgraph.semantic.core.relation_index().records() {
            if let Some(witness) = record.witness {
                out.push(witness.0);
            }
        }
        out
    }

    /// 在本 Session 定义表上求值（唯一 `ExecutionIR` 路径）。
    ///
    /// 返回归约后的 [`TermId`]。正式公共结果见 [`crate::api::AthenaEngine::execute_request`] → [`ComputationResult`]。
    pub fn evaluate(&mut self, expr: TermId) -> TermId {
        match execution::execute_ir_request(self, AthenaRequest::Term(expr)) {
            Ok(result_id) => self.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(expr),
            Err(_) => expr,
        }
    }

    /// 清除符号定义（不触及 heap / rings）。
    pub fn clear_definitions(&mut self) {
        self.defs.clear();
    }

    /// Session 运行时 heap。
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
    ) -> core::result::Result<(athena_graph::PublishedImmutableGraph<N, E>, athena_graph::CsrOnHeap), athena_graph::GraphError> {
        builder.finish_csr_on_heap(&mut self.heap.borrow_mut(), registry, budget)
    }

    /// 在 session 环表与多项式 DomainObject 仓中执行多项式域请求。
    pub fn execute_polynomial(&self, request: PolynomialRequest) -> PolynomialResult {
        execute_polynomial_with_rings(request, &self.rings, &self.polynomial_objects)
    }

    /// 经 M-Graph 缓存与 witness 记录执行多项式请求。
    pub fn execute_polynomial_mgraph(&mut self, request: PolynomialRequest) -> PolynomialResult {
        execute_polynomial_mgraph(request, &self.rings, &self.polynomial_objects, &mut self.mgraph)
    }

    /// 执行图论域请求（E0：与 [`crate::execute_domain`] 的 `GraphTheory` 分支等价）。
    pub fn execute_graph_theory(&self, request: GraphTheoryRequest) -> GraphTheoryResult {
        execute_graph_theory(request)
    }

    /// 执行线性代数域请求（经 `Session::matrix_objects` 解析）。
    pub fn execute_linear_algebra(&self, request: LinearAlgebraRequest) -> LinearAlgebraResult {
        execute_linear_algebra(request, &self.matrix_objects)
    }

    /// 经 `Session::groups` 执行群论域请求（可 intern）。
    pub fn execute_group(&mut self, request: GroupRequest) -> GroupResult {
        execute_group_with_table_mut(request, &mut self.groups)
    }

    /// 经 `Session::rings` 内嵌 [`crate::domains::algebra::FieldTable`] 执行域论请求（可 intern）。
    pub fn execute_field(&mut self, request: FieldRequest) -> FieldResult {
        execute_field_with_table_mut(request, self.rings.field_table_mut())
    }

    /// 经 field / group 表执行伽罗瓦域请求。
    pub fn execute_galois(&mut self, request: GaloisRequest) -> GaloisResult {
        execute_galois_with_tables(request, self.rings.field_table_mut(), &mut self.groups)
    }

    /// 在本 Session 的 scope-local E-Graph 上做预算内 saturation（只产候选，不写 M-Graph）。
    pub fn run_egraph_saturation(&mut self, roots: &[TermId], rules: Option<&RuleSet>) -> SaturationReport {
        saturate(&mut self.egraph, &self.arena, roots, self.egraph_budget, rules)
    }

    /// 类型化 [`TermPattern`] saturation（`match_pattern` + `substitute` · 不写 M-Graph）。
    pub fn run_egraph_saturation_typed(&mut self, roots: &[TermId], rules: Option<&TypedRuleSet>) -> SaturationReport {
        saturate_typed(&mut self.egraph, &mut self.arena, roots, self.egraph_budget, rules)
    }

    /// 从局部 e-class 提取代表项。
    pub fn extract_egraph_class(
        &self,
        class: crate::reasoning::egraph::EClassId,
        preference: crate::reasoning::egraph::ExtractionPreference,
    ) -> Option<TermId> {
        use crate::reasoning::egraph::Extractor;
        Extractor::with_preference(preference).extract(&self.egraph, &self.arena, class, Some(&self.mgraph.semantic.derived.exact_uf))
    }

    /// 提取代表项及其局部 [`crate::reasoning::egraph::ResultCost`]。
    pub fn extract_egraph_class_with_cost(
        &self,
        class: crate::reasoning::egraph::EClassId,
        preference: crate::reasoning::egraph::ExtractionPreference,
    ) -> Option<(TermId, crate::reasoning::egraph::ResultCost)> {
        use crate::reasoning::egraph::Extractor;
        Extractor::with_preference(preference).extract_with_cost(&self.egraph, &self.arena, class, Some(&self.mgraph.semantic.derived.exact_uf))
    }

    /// 局部 e-class 的多目标非支配提取集合。
    pub fn extract_egraph_class_pareto(&self, class: crate::reasoning::egraph::EClassId) -> crate::reasoning::egraph::ParetoFrontier {
        use crate::reasoning::egraph::Extractor;
        Extractor::extract_pareto(&self.egraph, &self.arena, class, Some(&self.mgraph.semantic.derived.exact_uf))
    }

    /// 经 TermStore 结构相等验证后接纳 `TermEquality`（写入 ExactUF + ProofForest）。
    pub fn admit_structural_term_equality(&mut self, left: TermId, right: TermId) -> Result<FactId, AdmissionRejectReason> {
        admit_structural_term_equality(&self.arena, &mut self.mgraph.semantic, left, right, &VerificationPolicy::default())
    }

    /// 将 E-Graph 候选升级为 M-Graph 事实（仅当结构相等时可接纳）。
    pub fn admit_egraph_candidate_if_structural(&mut self, candidate: &CandidateEquivalence) -> Result<FactId, AdmissionRejectReason> {
        self.admit_structural_term_equality(candidate.left_term, candidate.right_term)
    }

    /// 批量接纳 saturation 候选中结构相等的对（跳过改写型候选）。
    pub fn admit_structural_egraph_candidates(&mut self, candidates: &[CandidateEquivalence]) -> Vec<Result<FactId, AdmissionRejectReason>> {
        admit_structural_candidates(&self.arena, &mut self.mgraph.semantic, candidates, &VerificationPolicy::default())
    }

    /// 由已知 egraph 项发出 ExactUF 应用同余候选（不接纳）。
    pub fn emit_application_congruence_candidates(&self, max_pairs: u32) -> Vec<CandidateEquivalence> {
        application_congruence_candidates(&self.arena, &self.egraph, &self.mgraph.semantic.derived.exact_uf, max_pairs)
    }

    /// 当头相同且参数 ExactUF 相等时接纳 `f(a…) ≈ f(b…)`。
    pub fn admit_application_congruence(&mut self, left: TermId, right: TermId) -> Result<FactId, AdmissionRejectReason> {
        admit_application_congruence(&self.arena, &mut self.mgraph.semantic, left, right, &VerificationPolicy::default())
    }

    /// 在 ExactUF 下扫描已知应用并接纳同余等式（受 `max_pairs` 约束）。
    pub fn rebuild_and_admit_application_congruence(&mut self, max_pairs: u32) -> Vec<Result<FactId, AdmissionRejectReason>> {
        let candidates = application_congruence_candidates(&self.arena, &self.egraph, &self.mgraph.semantic.derived.exact_uf, max_pairs);
        admit_application_congruence_candidates(&self.arena, &mut self.mgraph.semantic, &candidates, &VerificationPolicy::default())
    }

    /// 类型化 saturation，再结构接纳、类型化改写回放接纳，最后 ExactUF 同余。
    ///
    /// 结构接纳只升级 `structural_eq` 对。改写候选需要 `rules`，以便回放重跑
    /// `match_pattern` + `substitute`。当 ExactUF 已使应用参数相等时触发同余。
    pub fn run_typed_egraph_admit_pipeline(
        &mut self,
        roots: &[TermId],
        rules: Option<&TypedRuleSet>,
        congruence_max_pairs: u32,
    ) -> TypedEgraphAdmitReport {
        let saturation = saturate_typed(&mut self.egraph, &mut self.arena, roots, self.egraph_budget, rules);
        let structural_admitted =
            admit_structural_candidates(&self.arena, &mut self.mgraph.semantic, &saturation.candidates, &VerificationPolicy::default());
        let rewrite_admitted = match rules {
            Some(rules) => admit_typed_rewrite_candidates(
                &mut self.arena,
                &mut self.mgraph.semantic,
                rules,
                &saturation.candidates,
                &VerificationPolicy::default(),
            ),
            None => Vec::new(),
        };
        let congruence_admitted = self.rebuild_and_admit_application_congruence(congruence_max_pairs);
        TypedEgraphAdmitReport { saturation, structural_admitted, rewrite_admitted, congruence_admitted }
    }

    /// 回放核验类型化改写候选（`match_pattern` + `substitute`）后接纳。
    pub fn admit_typed_rewrite_egraph_candidates(
        &mut self,
        rules: &TypedRuleSet,
        candidates: &[CandidateEquivalence],
    ) -> Vec<Result<FactId, AdmissionRejectReason>> {
        admit_typed_rewrite_candidates(&mut self.arena, &mut self.mgraph.semantic, rules, candidates, &VerificationPolicy::default())
    }

    /// 接纳无条件精确模同余（写入 modulus-isolated `CongruenceIndex`）。
    pub fn admit_congruence(&mut self, modulus_fingerprint: u64, left: u64, right: u64) -> Result<FactId, AdmissionRejectReason> {
        crate::reasoning::mgraph::AdmissionGate::admit_congruence(
            &mut self.mgraph.semantic,
            modulus_fingerprint,
            left,
            right,
            &VerificationPolicy::default(),
        )
    }

    /// 运行 M-Graph 相等森林闭包（传递性证明边 · bootstrap）。
    pub fn run_mgraph_closure(&mut self, limits: ClosureLimits) -> ClosureResult {
        self.mgraph.run_closure(&self.arena, &limits)
    }

    /// 将可暂存的运行态超边排入 OuterCandidate 池（不接纳）。
    pub fn drain_mgraph_hyper_edges(&mut self) -> crate::reasoning::mgraph::HyperEdgeDrainReport {
        crate::reasoning::mgraph::drain_hyper_edges_to_outer_pool(&self.arena, &mut self.mgraph)
    }

    /// 接纳通过 TermStore 结构相等的 OuterCandidate（升级为 ProvenExact）。
    pub fn admit_mgraph_outer_pool_if_structural(&mut self) -> crate::reasoning::mgraph::OuterAdmitReport {
        crate::reasoning::mgraph::admit_outer_pool_if_structural(&self.arena, &mut self.mgraph, &VerificationPolicy::default())
    }

    /// 登记挂起的 ProofObligation，供 Reflector 在接纳时唤醒。
    pub fn register_mgraph_obligation(&mut self, obligation: crate::reasoning::mgraph::ProofObligation) {
        self.mgraph.operational.obligation_index.register(obligation);
    }

    /// 将声明接纳进 M-Graph 状态并唤醒匹配义务。
    pub fn admit_mgraph_claim_with_wake(
        &mut self,
        claim: crate::reasoning::mgraph::Claim,
    ) -> Result<(crate::reasoning::mgraph::FactId, crate::reasoning::mgraph::WakeReport), AdmissionRejectReason> {
        crate::reasoning::mgraph::AdmissionGate::admit_claim_into_state(&mut self.mgraph, claim, &VerificationPolicy::default())
    }

    /// 将一批唤醒的 Reflector 结果写入运行态队列。
    pub fn schedule_mgraph_reflector_wakes(
        &mut self,
        wakes: &[crate::reasoning::mgraph::ReflectorWake],
        reflector: &dyn crate::reasoning::mgraph::SemanticReflector,
    ) -> crate::reasoning::mgraph::ReflectorScheduleReport {
        crate::reasoning::mgraph::schedule_reflector_wakes(&mut self.mgraph, wakes, reflector)
    }

    /// 从运行态 frontier 队列续跑未决义务。
    pub fn resume_mgraph_reflector_frontier(
        &mut self,
        reflector: &dyn crate::reasoning::mgraph::SemanticReflector,
    ) -> crate::reasoning::mgraph::ReflectorScheduleReport {
        crate::reasoning::mgraph::resume_reflector_frontier(&mut self.mgraph, reflector)
    }

    /// 用绑定的 [`crate::domains::DomainRequest`] 执行队首 `DomainPlan`。
    ///
    /// 精确微积分结果经 AdmissionGate 接纳。多项式路径用 `execute_polynomial_mgraph`
    /// （缓存 + AdmissionGate）。队列空时返回 `Ok(None)`。
    pub fn run_next_queued_domain_plan(
        &mut self,
        request: crate::domains::DomainRequest,
    ) -> Result<Option<crate::domains::DomainResult>, athena_types::Diagnostic> {
        crate::reasoning::mgraph::run_next_queued_plan(self, request)
    }

    /// 批量执行排队 DomainPlan，每条配绑定请求。
    pub fn run_queued_domain_plans(
        &mut self,
        requests: impl IntoIterator<Item = crate::domains::DomainRequest>,
    ) -> Result<crate::reasoning::mgraph::QueuedPlanBatchReport, athena_types::Diagnostic> {
        crate::reasoning::mgraph::run_queued_plans(self, requests)
    }
}
