//! 伽罗瓦域请求变体。

use athena_types::{ExtensionId, FieldId, SubgroupId};

use crate::domains::polynomial::Polynomial;

/// 伽罗瓦域请求。
///
/// **不**实现 [`Clone`]。含多项式载荷时用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum GaloisRequest {
    /// 多项式在基域上是否可分。
    IsPolynomialSeparable {
        /// 多项式。
        polynomial: Polynomial,
        /// 系数嵌入的基域。
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
    /// 扩张是否伽罗瓦（正规且可分）。
    IsGalois {
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
    /// 扩张的伽罗瓦群。
    GaloisGroupOfExtension {
        /// 扩张 id。
        extension: ExtensionId,
    },
    /// 固定域：自同构子群对应的中间域。
    FixedField {
        /// 扩张 id。
        extension: ExtensionId,
        /// 自同构子群。
        automorphism_subgroup: SubgroupId,
    },
}

impl GaloisRequest {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::IsPolynomialSeparable { polynomial, base_field } => {
                Self::IsPolynomialSeparable { polynomial: polynomial.owning_copy(), base_field: *base_field }
            }
            Self::IsExtensionNormal { extension } => Self::IsExtensionNormal { extension: *extension },
            Self::IsExtensionSeparable { extension } => Self::IsExtensionSeparable { extension: *extension },
            Self::IsGalois { extension } => Self::IsGalois { extension: *extension },
            Self::GaloisGroupOfPolynomial { polynomial, base_field } => {
                Self::GaloisGroupOfPolynomial { polynomial: polynomial.owning_copy(), base_field: *base_field }
            }
            Self::GaloisGroupOfExtension { extension } => Self::GaloisGroupOfExtension { extension: *extension },
            Self::FixedField { extension, automorphism_subgroup } => {
                Self::FixedField { extension: *extension, automorphism_subgroup: *automorphism_subgroup }
            }
        }
    }
}
