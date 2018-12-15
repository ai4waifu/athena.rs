//! M-Graph **形式化理论**（crate 内镜像 · **非运行时**）。
//!
//! 本节是理论层完整规格的 crate 内摘要；**下列签名、算子、公理不得直接物化为 `struct` / 强制 trait。**
//!
//! # 第一原则
//!
//! ```text
//! P0  先把 M-Graph 缩到不可再删，再证明它足够表达闭包
//! P1  根源宇宙 𝒰 是 ambient 背景，不是运行时对象
//! P2  对象身份由关系决定；M-Graph 只索引关系，不拥有领域对象本体
//! ```
//!
//! # 理论签名（不实现）
//!
//! ```text
//! 𝕄_theory = (𝒰, 𝓦, 𝓕, ∂, Σ, T, A, C, Π)
//!
//! 𝒰   根源宇宙 — 元层背景（类似 A : U 中的 U）
//! 𝓦   scope / 内世界索引（局部语义上下文 w = (A, B, D)）
//! 𝓕   纤维化：w ↦ 𝓕(w) ⊆ Rel(w)  已接纳关系族
//! ∂   边界：Outer ⇄ Inner  （interpret · verify · admit）
//! Σ   ScopeRelation：Refines · Restricts · Compatible · Incompatible
//! T   transport 沿 Σ 的规则驱动关系迁移
//! A   admission：VerifiedClaim → 单调扩展 I_rel
//! C   closure Cl：仅对已 admit 的 RelationRef 传播
//! Π   projection：StableFact · BranchFact · ConditionalFact（查询，非节点）
//! ```
//!
//! **内 / 外（相对，非容器）**
//!
//! ```text
//! Inner = ⋃_{w∈𝓦} 𝓕(w) \ Refuted
//! Outer = CandidatePool / OuterCandidate（栈上或 operational）
//! r|𝓦 ∈ Rel(𝓦)           内化
//! r ∈ Rel(𝒰) \ Rel(𝓦)    外候选
//! ```
//!
//! # 实现签名（`MGraphCore`）
//!
//! ```text
//! 𝕄_impl = (I_scope, I_rel, T_idx, S_close)
//!
//! I_scope   ScopeIndex      ScopeRef → ScopeEdge*
//! I_rel     RelationIndex   RelationRef → RelationRecord
//! T_idx     （最小 transport 表，可扩展）
//! S_close   ClosureSeeds    close(seeds) 入口
//! ```
//!
//! # 理论 — 实现对照
//!
//! | 理论 | Rust 类型 | 禁止 |
//! |------|-----------|------|
//! | 𝒰 | （无） | `RootUniverse` |
//! | w ∈ 𝓦 | `ScopeRef` | 全量 `World` struct |
//! | r ∈ 𝓕(w) | `RelationRef` + `RelationRecord` | 第二套 God DAG |
//! | Σ | `ScopeEdge` / `ScopeRelationKind` | world registry |
//! | Outer | `OuterCandidate` | `OuterWorld` |
//! | A | `MGraphCore::admit` | 绕过 `EvidenceVerifier` |
//! | C | `MGraphCore::close` | 闭包队列进 claim |
//! | Π | `MGraphView` + ExtractionState | `PrincipalProjection` 节点 |
//! | EGraph(w) | `DerivedIndexes` | E-Graph 进 SemanticCore |
//!
//! # 判词形式
//!
//! ```text
//! Γ ⊢ φ @ w                 可陈述
//! Γ ⊢ admit(r) @ w          verifier 后可写入
//! 𝕄 ⊢ w₁ ⊑ w₂               ScopeRelation::Refines
//! 𝕄 ⊢ transport(r, w₁→w₂)   规则驱动，非 world 复制
//! ```
//!
//! # 算子语义（摘要）
//!
//! **Admission**
//!
//! ```text
//! A(𝕄, vc) = 𝕄'  若 verify(vc) = Admitted
//! I_rel' = I_rel ∪ { record(vc) }     单调追加，不可撤销
//! ```
//!
//! **Closure**
//!
//! ```text
//! Cl(F₀) = ⋃_n F_n   F_{n+1} = F_n ∪ one_step_T(F_n)
//! extensive · monotone · idempotent（有限 fragment）
//! close 不将 Candidate 当作事实
//! ```
//!
//! **Projection（查询）**
//!
//! ```text
//! StableFact(V)  = ⋂_{w∈V} Accepted 纤维
//! BranchFact(V)  = ⋃ 𝓕(w) \ StableFact
//! 无 MainWorld — 禁止 argmax(w) 选主世界
//! ```
//!
//! # 十条公理 A1–A10
//!
//! 1. **极小本体** — 删后仍可判 merge/传播 → 属 SemanticCore
//! 2. **单调 admit** — 只追加 `I_rel`
//! 3. **scope 局部性** — 每条记录恰属一个 `ScopeRef`
//! 4. **非拥有对象** — 仅 `*Ref`，无领域对象副本
//! 5. **外候选隔离** — `OuterCandidate` ∉ SemanticCore
//! 6. **transport 显式** — 仅沿注册 `ScopeRelation`
//! 7. **闭包保真** — `Cl` 不伪造 witness
//! 8. **未证 ≠ 反证** —  absent ≠ Refuted
//! 9. **无语义代价** — cost/trace/cache 不进 claim
//! 10. **𝒰 不实例化** — 无 RootUniverse / 全局对象表
//!
//! # Galois（框架，非许可证）
//!
//! ```text
//! α : ConcreteExecution → MinimalClaim
//! γ : AbstractClaim × Policy → ConcretePlans   # Planner 外围
//! α(c) ⊑ a  ⇔  c ⊑ γ(a)
//! ```
//!
//! Verifier + admit 才是 Inner 入口；α  alone 不够。
//!
//! # 范畴论禁令
//!
//! `Category` / `Functor` / `Fibration` **不得**作为 mgraph 强制 trait。
//! 范畴语言只约束：可组合性 · 结构保持 · transport 合法性。
//!
//! # 压缩公理
//!
//! ```text
//! 根源宇宙是背景，不是对象。
//! 内世界是 scope，不是复制的完整模型。
//! 外世界是候选来源，不是全量容器。
//! M-Graph = scoped relation index + admission + closure
//! ```
