//! Typed SSA operations (closed opcode family — no string handler lookup).

use athena_types::OperatorId;

use super::ids::{CapturedRootId, ConstantId, EffectToken, InputId, ProviderCallId, SsaValueId};
use super::types::ExecutionValueType;

/// One SSA operation definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// Result value defined by this operation (`None` for unit-only ops).
    pub result: Option<SsaValueId>,
    /// Static result type when `result` is present.
    pub result_type: ExecutionValueType,
    /// Opcode payload.
    pub kind: OperationKind,
    /// Required incoming effect token when the opcode is effectful.
    pub effect_in: Option<EffectToken>,
    /// Effect token produced when the opcode is effectful.
    pub effect_out: Option<EffectToken>,
}

/// Closed semantic opcode set for `ExecutionIR`.
///
/// Dialect surface names (`Hold`, `RuleDelayed`, `Plus`, …) must not appear here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    /// Load a module input into an SSA value.
    LoadInput {
        /// Input table index.
        input: InputId,
    },
    /// Load an immutable term root (read-only handle).
    LoadTerm {
        /// Captured term root.
        root: CapturedRootId,
    },
    /// Materialize a module constant.
    Constant {
        /// Constant table index.
        constant: ConstantId,
    },
    /// Apply a closed semantic operator to SSA arguments.
    ApplySemanticOperator {
        /// Neutral operator identity.
        operator: OperatorId,
        /// Operand SSA values.
        args: Vec<SsaValueId>,
    },
    /// Build a `TermNode::List` from evaluated element SSA values.
    MakeList {
        /// Element SSA values (order preserved).
        elements: Vec<SsaValueId>,
    },
    /// Read a Session / scope binding.
    ReadBinding {
        /// Binding key SSA value (symbol / slot handle).
        key: SsaValueId,
    },
    /// Write a Session / scope binding.
    WriteBinding {
        /// Binding key.
        key: SsaValueId,
        /// Value written.
        value: SsaValueId,
        /// When true, store as Delayed OwnValues (evaluate on read).
        delayed: bool,
    },
    /// Append a DownValue rule (`f[pat] := rhs`).
    WriteDownValue {
        /// Head symbol key.
        key: SsaValueId,
        /// Full pattern lhs term (usually `f[…]`).
        pattern: SsaValueId,
        /// Deferred rhs term (not evaluated at write).
        value: SsaValueId,
    },
    /// Enter a lexical or dynamic scope frame.
    EnterScope {
        /// Optional parent scope SSA handle.
        parent: Option<SsaValueId>,
    },
    /// Exit the current scope frame.
    ExitScope {
        /// Scope handle produced by [`Self::EnterScope`].
        scope: SsaValueId,
    },
    /// Typed provider call.
    CallProvider {
        /// Descriptor table index.
        call: ProviderCallId,
        /// Argument SSA values.
        args: Vec<SsaValueId>,
    },
    /// Guard that may take an explicit exit edge.
    Guard {
        /// Predicate SSA value (typed Boolean).
        predicate: SsaValueId,
        /// Success continues in-block; failure uses terminator / exit tables.
        on_failure: GuardFailure,
    },
    /// Materialize a runtime `Value` from SSA / term handles.
    MaterializeValue {
        /// Source SSA value.
        source: SsaValueId,
    },
    /// Publish into `ResultStore`.
    PublishResult {
        /// Value or residual to publish.
        source: SsaValueId,
    },
}

/// Guard failure routing (success falls through).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardFailure {
    /// Route to a declared module exit.
    Exit(super::ids::ExitId),
    /// Reject the current region immediately via terminator contract.
    Reject,
}

impl Operation {
    /// Pure constant load.
    pub fn constant(result: SsaValueId, constant: ConstantId) -> Self {
        Self {
            result: Some(result),
            result_type: ExecutionValueType::Unknown,
            kind: OperationKind::Constant { constant },
            effect_in: None,
            effect_out: None,
        }
    }

    /// Pure term load.
    pub fn load_term(result: SsaValueId, root: CapturedRootId) -> Self {
        Self {
            result: Some(result),
            result_type: ExecutionValueType::Term,
            kind: OperationKind::LoadTerm { root },
            effect_in: None,
            effect_out: None,
        }
    }
}
