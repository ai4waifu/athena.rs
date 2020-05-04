//! 稳定标识符 newtype（IR 与注册表）。

/// 不可变符号项身份（`TermStore` 原生引用，不是二级映射句柄）。
///
/// 已计算值用 [`ValueId`]；结果容器用 [`ResultId`]。禁止与二者互换。
///
/// **生命周期**：裸 [`TermId`] 仅在所属 store 的当前 [`TermRef::generation`] 下有效。
/// 跨执行 / 缓存边界应携带 [`TermRef`]（index + generation）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TermId(pub u32);

/// 带 generation 的项句柄（防 ABA · 对齐 `GcObjectId` 合同）。
///
/// 过渡期：`generation` 取自 `TermStore` epoch（整库代际）。
/// TermStore GC 闭合后可演进为 per-slot generation，而不改公共字段名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TermRef {
    /// [`TermId`] 下标。
    pub id: TermId,
    /// Store / 槽代际（须与 `TermStore::epoch` 一致方可解引用）。
    pub generation: u32,
}

impl TermRef {
    /// 构造。
    #[inline]
    pub const fn new(id: TermId, generation: u32) -> Self {
        Self { id, generation }
    }
}

/// 符号 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub u32);

/// 扩展算子身份（仅扩展显示名 / apply 索引，禁止承担核心数学语义）。
///
/// 核心算子用 [`athena_ir::SemanticOperator`]，不得经本 id 做算术 / 微积分分派。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExtensionOperatorId(pub u32);

/// 数学域 id（系数域等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DomainId(pub u32);

/// 群对象 id（**Session-local** 查找句柄；跨 Session 用 fingerprint）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId(pub u32);

/// 群元素 id（绑定所属群，禁止跨群运算）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupElementId(pub u32);

/// 域对象 id（**Session-local** 查找句柄；跨 Session 用 fingerprint）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldId(pub u32);

/// 域扩张 id（**Session-local**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExtensionId(pub u32);

/// 系数环 id（ℤ / ℚ / 𝔽_p / ℤ/nℤ / 有限域等；Session 内 intern 句柄）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoefficientRingId(pub u32);

/// 多项式环 id（**Session-local** 查找句柄；跨 Session 语义身份为 `RingFingerprint`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RingId(pub u32);

/// 擦除后的 presentation 句柄（仅跨域共享骨架；新代码用强类型）。
///
/// Session-local：数值相等不代表跨 Session 同一表示。跨 Session / 缓存 / 序列化用 fingerprint。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PresentationId(pub u32);

/// 域 presentation 句柄（Session-local；禁止与 [`GroupPresentationId`] 混用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldPresentationId(pub u32);

/// 群 presentation 句柄（Session-local；禁止与 [`FieldPresentationId`] 混用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupPresentationId(pub u32);

/// 代数映射 id（嵌入、同态、商投影等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AlgebraMapId(pub u32);

/// 域自同构 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AutomorphismId(pub u32);

/// 子群 id（含 inclusion 映射引用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubgroupId(pub u32);

/// 假设集合 id（Session / 请求附着；过渡期与 [AssumptionScopeId] 并存）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssumptionSetId(pub u32);

/// 假设作用域 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssumptionScopeId(pub u32);

/// 理论 / 猜想上下文 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TheoryContextId(pub u32);

/// 表面语法身份（产品层 / 方言 Form；非 Athena 语义表达式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FormId(pub u32);

/// 已计算或已验证的值身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(pub u32);

/// 结果容器身份（解集 / 条件结果等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResultId(pub u32);

/// 可恢复计算前沿身份（`FrontierStore` · Session-local）。
///
/// 不得与 [`TermId`] / [`ValueId`] / [`ResultId`] 互换。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrontierId(pub u32);

/// 证明 / 证据引用身份。
///
/// 不得混入表达式 equality，也不得与 [TermId] / [ValueId] / [ResultId] 互换。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProofRef(pub u64);

/// 多项式对象身份（预留；对象表在 engine）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PolynomialId(pub u32);

/// 矩阵对象身份（预留；对象表在 engine）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MatrixId(pub u32);

/// 源码位置（字节偏移）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceSpan {
    /// 起始字节（含）。
    pub start: u32,
    /// 结束字节（不含）。
    pub end: u32,
}

/// IR / wire 序列化 schema 版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SerializationVersion(pub u16);

impl SerializationVersion {
    /// 当前 schema。
    pub const CURRENT: Self = Self(1);
}
