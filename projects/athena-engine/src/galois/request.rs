//! 伽罗瓦域请求。

use athena_types::{ExtensionId, FieldId, SubgroupId};

use crate::polynomial::Polynomial;

/// 伽罗瓦域请求（骨架）。
#[derive(Debug, Clone, PartialEq)]
pub enum GaloisRequest {
    /// 多项式在基域上是否可分。
    IsPolynomialSeparable {
        /// 多项式。
        polynomial: Polynomial,
        /// 基域。
        base_field: FieldId,
    },
    /// 构造分裂域（骨架）。
    SplittingField {
        /// 多项式。
        polynomial: Polynomial,
        /// 基域。
        base_field: FieldId,
    },
    /// 扩张是否正规。
    IsExtensionNormal {
        /// 扩张 id。
        extension: ExtensionId,
    },
    /// 扩张是否可分。
    IsExtensionSeparable {
        /// 扩张 id。
        extension: ExtensionId,
    },
    /// 扩张是否伽罗瓦（可分且正规）。
    IsGalois {
        /// 扩张 id。
        extension: ExtensionId,
    },
    /// 扩张的伽罗瓦群。
    GaloisGroupOfExtension {
        /// 扩张 id。
        extension: ExtensionId,
    },
    /// 多项式分裂域相对基域的伽罗瓦群。
    GaloisGroupOfPolynomial {
        /// 多项式。
        polynomial: Polynomial,
        /// 基域。
        base_field: FieldId,
    },
    /// 固定域。
    FixedField {
        /// 扩张。
        extension: ExtensionId,
        /// 自同构子群。
        automorphism_subgroup: SubgroupId,
    },
}
