//! `MatrixResult` — Living 16 domain envelope for matrix-valued outcomes.
//!
//! Session [`crate::runtime::results::ComputationResult`] still owns `ResultId` /
//! `derived_from` / host projection. This type freezes the **domain-local** field
//! closed set so shape, element domain, guarantee, residual, and diagnostics travel
//! with the matrix value instead of being reconstructed from display text.

use athena_types::Diagnostic;

use super::{
    object_ref::MatrixRef,
    parent::ElementParentKind,
    shape::MatrixShape,
    status::AlgorithmGuarantee,
    value::MatrixValue,
};

/// Domain-local matrix outcome (subset of Living 16 `MatrixResult`).
///
/// **Not** [`Clone`]. Deep copy via [`Self::owning_copy`].
#[derive(Debug, PartialEq)]
pub struct MatrixResult {
    /// Owned matrix payload (primary value carrier for this slice).
    pub value: MatrixValue,
    /// Optional session store handle when the value is also interned.
    pub matrix_ref: Option<MatrixRef>,
    /// Shape snapshot (must match `value.shape()`).
    pub shape: MatrixShape,
    /// Element parent kind snapshot (must match `value.parent().element`).
    pub element_domain: ElementParentKind,
    /// Algorithm guarantee for this outcome.
    pub guarantee: AlgorithmGuarantee,
    /// Optional `‖residual‖_∞` (machine paths).
    pub residual_inf: Option<f64>,
    /// Optional conditioning estimate (machine paths).
    pub conditioning: Option<f64>,
    /// Structured diagnostics carried with the outcome.
    pub diagnostics: Vec<Diagnostic>,
    /// Store revision when `matrix_ref` is set; `None` for ephemeral owned values.
    pub revision: Option<u64>,
}

impl MatrixResult {
    /// Build an owned exact/approximate envelope from a matrix value.
    pub fn from_owned(value: MatrixValue, guarantee: AlgorithmGuarantee) -> Self {
        let shape = value.shape();
        let element_domain = value.parent().element;
        Self {
            value,
            matrix_ref: None,
            shape,
            element_domain,
            guarantee,
            residual_inf: None,
            conditioning: None,
            diagnostics: Vec::new(),
            revision: None,
        }
    }

    /// Attach a store handle + revision without changing the owned payload.
    pub fn with_ref(mut self, matrix_ref: MatrixRef, revision: u64) -> Self {
        self.matrix_ref = Some(matrix_ref);
        self.revision = Some(revision);
        self
    }

    /// Attach machine residual / conditioning witnesses.
    pub fn with_machine_witness(mut self, residual_inf: f64, conditioning: Option<f64>) -> Self {
        self.residual_inf = Some(residual_inf);
        self.conditioning = conditioning;
        self
    }

    /// Owning copy (diagnostics cloned; matrix via [`MatrixValue::owning_copy`]).
    pub fn owning_copy(&self) -> Self {
        Self {
            value: self.value.owning_copy(),
            matrix_ref: self.matrix_ref,
            shape: self.shape,
            element_domain: self.element_domain,
            guarantee: self.guarantee,
            residual_inf: self.residual_inf,
            conditioning: self.conditioning,
            diagnostics: self.diagnostics.clone(),
            revision: self.revision,
        }
    }
}
