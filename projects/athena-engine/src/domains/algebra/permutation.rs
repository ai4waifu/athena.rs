//! 置换运算（合成约定 `compose(p, q)(i) = p(q(i))`）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

/// 内部置换（像列表 `π(i) = images[i]`）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct RawPerm {
    images: Vec<u32>,
}

impl RawPerm {
    /// 构造并校验双射。
    pub fn new(images: Vec<u32>, degree: u32) -> Result<Self> {
        validate_images(&images, degree)?;
        Ok(Self { images })
    }

    /// Owning 复制（Living `31`：像向量）。
    pub fn owning_copy(&self) -> Self {
        Self { images: self.images.clone() }
    }

    /// 像列表（长度 = degree）。
    pub fn images(&self) -> &[u32] {
        &self.images
    }

    /// 度数。
    pub fn degree(&self) -> u32 {
        self.images.len() as u32
    }

    /// 单位置换。
    pub fn identity(degree: u32) -> Self {
        Self { images: (0..degree).collect() }
    }

    /// 是否单位元。
    pub fn is_identity(&self) -> bool {
        self.images.iter().enumerate().all(|(i, &j)| i as u32 == j)
    }

    /// 应用 `π(i)`。
    pub fn apply(&self, point: u32) -> u32 {
        self.images[point as usize]
    }

    /// 合成 `p(q(i))`。
    pub fn compose(&self, other: &Self) -> Result<Self> {
        if self.degree() != other.degree() {
            return Err(permutation_invalid("degree_mismatch"));
        }
        let n = self.degree() as usize;
        let images: Vec<u32> = (0..n).map(|i| self.apply(other.apply(i as u32))).collect();
        Ok(Self { images })
    }

    /// 逆元。
    pub fn inverse(&self) -> Self {
        let n = self.images.len();
        let mut inv = vec![0u32; n];
        for (i, &j) in self.images.iter().enumerate() {
            inv[j as usize] = i as u32;
        }
        Self { images: inv }
    }
}

/// 校验置换像。
pub fn validate_images(images: &[u32], degree: u32) -> Result<()> {
    if images.len() != degree as usize {
        return Err(permutation_invalid("image_length"));
    }
    let mut seen = vec![false; degree as usize];
    for &j in images {
        if j >= degree {
            return Err(permutation_invalid("image_out_of_range"));
        }
        if seen[j as usize] {
            return Err(permutation_invalid("not_bijective"));
        }
        seen[j as usize] = true;
    }
    Ok(())
}

fn permutation_invalid(operation: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::PermutationInvalid).detail("domain", "group").detail("operation", operation)
}
