//! 线性代数领域模块（`athena-engine` 内）。
//!
//! 矩阵父对象 / shape / layout / `IndexSpec` / 构造与基础算子。
//! 精确（`ℚ` 消元 · Bareiss）与机器（部分主元 LU）双路径。
//! 不包含方言表面算子；方言 lowering 属于 SXO。

mod equality;
mod exact;
mod index;
mod machine;
mod ops;
mod parent;
mod request;
mod result;
mod shape;
mod status;
mod value;

pub use equality::{MatrixEqualityKind, matrices_equal};
pub use exact::{ExactDetResult, ExactRankResult, ExactRrefResult, ExactSolveResult, det_bareiss, rank_exact, rref_rational, solve_exact};
pub use index::{AxisRange, IndexSpec, scalar_index_from_one_based, slice_index_from_one_based_inclusive};
pub use machine::{MachineLuFactorization, MachineSolveResult, lu_partial_pivot, rank_machine, solve_lu, solve_machine};
pub use ops::{hadamard, index_scalar, matmul, slice_matrix, transpose};
pub use parent::{ElementParentKind, MatrixParent, RoundingPolicy, ShapePolicy, SparseStrategy};
pub use request::LinearAlgebraRequest;
pub use result::{DEFAULT_PIVOT_THRESHOLD, LinearAlgebraResult, LinearAlgebraValue, execute_linear_algebra, operation_name};
pub use shape::{Layout, MatrixShape, StorageOrder};
pub use status::{AlgorithmGuarantee, MachineSolveWitness, SolveDisposition};
pub use value::{MatrixBuffer, MatrixEntry, MatrixValue};
