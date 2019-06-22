//! Static types carried by SSA values inside `ExecutionIR`.

use athena_types::{OperatorId, ResultId, TermId, ValueId};

use super::ids::{CapturedRootId, ConstantId, InputId, ProviderCallId};

/// Closed value-type lattice for SSA values.
///
/// Identities from other Athena domains appear only as typed handles, never as
/// overlapping id namespaces with [`super::ids::SsaValueId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionValueType {
    /// Untyped / not yet constrained (verifier rejects in final modules).
    Unknown,
    /// Typed Boolean.
    Boolean,
    /// Symbolic term handle (`TermStore` identity).
    Term,
    /// Runtime value handle (`ValueStore` identity).
    Value,
    /// Published computation result handle.
    Result,
    /// Opaque provider payload handle.
    ProviderPayload,
    /// Unit / void (side-effect-only operations).
    Unit,
}

/// Module-level constant payload (compile-time known).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstantValue {
    /// Boolean literal.
    Boolean(bool),
    /// Interned term root already present in `TermStore`.
    Term(TermId),
    /// Unit constant.
    Unit,
}

/// Module input binding (request / snapshot edge).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInput {
    /// Stable input slot.
    pub id: InputId,
    /// Static type of the input SSA value.
    pub ty: ExecutionValueType,
}

/// Captured GC / Session root referenced by the module (not owned by IR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapturedRoot {
    /// TermStore node root.
    Term(TermId),
    /// ValueStore object root.
    Value(ValueId),
    /// ResultStore entry root.
    Result(ResultId),
}

/// Descriptor for a typed provider call site (language-neutral).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCallDescriptor {
    /// Descriptor table index.
    pub id: ProviderCallId,
    /// Closed semantic operator identity (not a dialect surface name).
    pub operator: OperatorId,
    /// Expected argument types.
    pub argument_types: Vec<ExecutionValueType>,
    /// Result type.
    pub result_type: ExecutionValueType,
    /// Whether the call is a GC / budget / cancellation safepoint.
    pub safepoint: bool,
}

/// Convenience constructors for module tables.
impl ConstantValue {
    /// Boolean constant.
    pub fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    /// Term constant.
    pub fn term(term: TermId) -> Self {
        Self::Term(term)
    }
}

impl ModuleInput {
    /// Typed module input.
    pub fn new(id: InputId, ty: ExecutionValueType) -> Self {
        Self { id, ty }
    }
}

impl CapturedRoot {
    /// Wrap a term root.
    pub fn term(term: TermId) -> Self {
        Self::Term(term)
    }
}

impl ProviderCallDescriptor {
    /// Minimal provider descriptor.
    pub fn new(id: ProviderCallId, operator: OperatorId, result_type: ExecutionValueType) -> Self {
        Self {
            id,
            operator,
            argument_types: Vec::new(),
            result_type,
            safepoint: true,
        }
    }
}

/// Resolve table ids used by freeze tests / builders.
pub fn unused_constant_id() -> ConstantId {
    ConstantId(0)
}

/// Resolve captured-root table id used by freeze tests / builders.
pub fn unused_captured_root_id() -> CapturedRootId {
    CapturedRootId(0)
}
