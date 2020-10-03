//! 线性代数请求中的矩阵操作数（对象句柄或符号绑定）。

use athena_types::{Diagnostic, DiagnosticCode, SymbolId};

use super::object_ref::{MatrixObjectStore, MatrixRef};
use super::value::MatrixValue;

/// 矩阵操作数：已 intern 的 DomainObject，或执行期解析的符号绑定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatrixOperand {
    /// Session [`MatrixObjectStore`] 句柄。
    Object(MatrixRef),
    /// [`crate::execution::environment::DefinitionLayer`] 矩阵绑定键。
    Binding(SymbolId),
}

impl From<MatrixRef> for MatrixOperand {
    fn from(matrix: MatrixRef) -> Self {
        Self::Object(matrix)
    }
}

impl MatrixOperand {
    /// 已 intern 对象。
    pub const fn object(matrix: MatrixRef) -> Self {
        Self::Object(matrix)
    }

    /// 符号矩阵绑定（执行期解析）。
    pub const fn binding(symbol: SymbolId) -> Self {
        Self::Binding(symbol)
    }

    /// 解析为拥有的 [`MatrixValue`]。
    pub fn resolve_value(
        self,
        store: &MatrixObjectStore,
        matrix_binding: &dyn Fn(SymbolId) -> Option<MatrixRef>,
    ) -> Result<MatrixValue, Diagnostic> {
        let matrix_ref = self.resolve_ref(store, matrix_binding)?;
        store.resolve_owning(matrix_ref).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "linear_algebra")
                .detail("reason", "missing_matrix_ref")
                .arg("ref", matrix_ref.0)
        })
    }

    /// 解析为 [`MatrixRef`]（Binding 经定义层查找）。
    pub fn resolve_ref(
        self,
        store: &MatrixObjectStore,
        matrix_binding: &dyn Fn(SymbolId) -> Option<MatrixRef>,
    ) -> Result<MatrixRef, Diagnostic> {
        match self {
            Self::Object(matrix) => {
                if store.get(matrix).is_some() {
                    Ok(matrix)
                } else {
                    Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("domain", "linear_algebra")
                        .detail("reason", "missing_matrix_ref")
                        .arg("ref", matrix.0))
                }
            }
            Self::Binding(symbol) => matrix_binding(symbol).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "linear_algebra")
                    .detail("reason", "missing_matrix_binding")
                    .arg("symbol", symbol.0)
            }),
        }
    }
}
