//! athena 生态基础类型 — 仅合同，无求值 / 解析 / IO。
//!
//! 数值表示与运算在 [`athena_numeric`]；本 crate 不拥有 `NumericDomain` / 执行态 `Number`。

#![deny(missing_docs)]

mod assumption;
mod diagnostic;
mod ids;
mod numeric_kind;
mod scope;
mod status;
/// 过渡期宿主 wire（十进制字符串）；执行路径请用 `athena_numeric::NumericValue`。
pub mod wire;

pub use assumption::{AssumptionSet, Condition, Predicate};
pub use diagnostic::{Diagnostic, DiagnosticCode, DiagnosticPath, DiagnosticValue, Result, Severity};
pub use ids::{
    AlgebraMapId, AssumptionScopeId, AssumptionSetId, AutomorphismId, CoefficientRingId, DomainId, ExtensionId, FieldId,
    FieldPresentationId, FormId, GroupElementId, GroupId, GroupPresentationId, MatrixId, OperatorId, PolynomialId,
    PresentationId, ProofRef, ResultId, RingId, SerializationVersion, SourceSpan, SubgroupId, SymbolId, TermId,
    TheoryContextId, ValueId,
};
pub use numeric_kind::{ModulusId, NumericKind, NumericTypeId, PrecisionPolicyId};
pub use scope::{
    AssumptionBranchPolicy, AssumptionScope, ScopeApplicability, ScopeConflict, ScopeConflictKind, ScopeMergeOutcome,
    TheoryContext,
};
pub use status::ComputationStatus;
pub use wire::{ExactNumber, RealNumber, WireNumber};

/// 近似算术的舍入模式（合同）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingMode {
    /// 四舍六入五成双（最近偶数）。
    #[default]
    Nearest,
    /// 向零舍入。
    Truncate,
    /// 向 +∞。
    Ceiling,
    /// 向 -∞。
    Floor,
}

/// 精度策略（合同）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// 精确算术。
    Exact,
    /// IEEE binary64。
    Machine,
    /// 任意比特精度。
    ArbitraryBits(u32),
}
