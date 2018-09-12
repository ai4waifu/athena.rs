//! 伽罗瓦域请求。

use athena_types::FieldId;

use crate::polynomial::Polynomial;

/// 伽罗瓦域请求（骨架）。
#[derive(Debug, Clone, PartialEq)]
pub enum GaloisRequest {
    /// 是否可分。
    IsSeparable {
        /// 多项式。
        polynomial: Polynomial,
        /// 基域。
        base_field: FieldId,
    },
    /// 是否正规扩张 / 分裂域（骨架合一入口）。
    IsNormal {
        /// 多项式。
        polynomial: Polynomial,
        /// 基域。
        base_field: FieldId,
    },
    /// 伽罗瓦群。
    GaloisGroup {
        /// 多项式。
        polynomial: Polynomial,
        /// 基域。
        base_field: FieldId,
    },
    /// 固定域（骨架）。
    FixedField {
        /// 自同构标签集合（后续换 id）。
        automorphism_labels: Vec<String>,
        /// 基域。
        base_field: FieldId,
    },
}
