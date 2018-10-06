//! 单项式序 — 属于 [`super::ring::RingDescriptor`] 身份，不是算法临时选项。

use std::cmp::Ordering;

use athena_types::{Diagnostic, DiagnosticCode};

/// 单项式比较序（环身份的一部分）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MonomialOrder {
    /// 字典序（变量表顺序即 `x0 > x1 > …`）。
    Lex,
    /// 先总次数，再字典序。
    GrLex,
    /// 先总次数，再逆字典序（从右变量开始）。
    GrevLex,
    /// 加权次数 `Σ w_i e_i`，再字典序破 tie。
    Weighted {
        /// 与变量表等长的权重。
        weights: Vec<u32>,
    },
    /// 分块序：每块等宽变量子序列，块内递归应用对应序。
    Block {
        /// 每块变量数，之和须等于环变量数。
        blocks: Vec<MonomialOrder>,
    },
    /// 消元序：前 `eliminate` 个变量按 Lex，其余按 `rest`。
    Elimination {
        /// 消元块宽度。
        eliminate: u32,
        /// 剩余变量序。
        rest: Box<MonomialOrder>,
    },
}

impl MonomialOrder {
    /// 校验序与变量数是否一致（在环构造时调用）。
    pub fn validate_for_variables(&self, variable_count: usize) -> Result<(), Diagnostic> {
        match self {
            Self::Lex | Self::GrLex | Self::GrevLex => Ok(()),
            Self::Weighted { weights } => {
                if weights.len() == variable_count {
                    Ok(())
                }
                else {
                    Err(order_invalid("weighted_length"))
                }
            }
            Self::Block { blocks } => {
                if blocks.is_empty() || variable_count == 0 {
                    return Err(order_invalid("block_empty"));
                }
                if variable_count % blocks.len() != 0 {
                    return Err(order_invalid("block_width"));
                }
                let w = variable_count / blocks.len();
                for b in blocks {
                    b.validate_for_variables(w)?;
                }
                Ok(())
            }
            Self::Elimination { eliminate, rest } => {
                let e = *eliminate as usize;
                if e > variable_count {
                    return Err(order_invalid("elimination_width"));
                }
                rest.validate_for_variables(variable_count.saturating_sub(e))?;
                Ok(())
            }
        }
    }

    /// 比较两个指数向量（长度须等于 `variable_count`）。
    pub fn cmp_exponents(&self, a: &[u32], b: &[u32], variable_count: usize) -> Result<Ordering, Diagnostic> {
        if a.len() != variable_count || b.len() != variable_count {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "cmp_exponents"));
        }
        self.validate_for_variables(variable_count)?;
        cmp_exponents_inner(a, b, self)
    }
}

fn cmp_exponents_inner(a: &[u32], b: &[u32], order: &MonomialOrder) -> Result<Ordering, Diagnostic> {
    match order {
        MonomialOrder::Lex => Ok(cmp_lex(a, b)),
        MonomialOrder::GrLex => {
            let da = total_degree(a);
            let db = total_degree(b);
            match da.cmp(&db) {
                Ordering::Equal => Ok(cmp_lex(a, b)),
                other => Ok(other),
            }
        }
        MonomialOrder::GrevLex => {
            let da = total_degree(a);
            let db = total_degree(b);
            match da.cmp(&db) {
                Ordering::Equal => Ok(cmp_grevlex(a, b)),
                other => Ok(other),
            }
        }
        MonomialOrder::Weighted { weights } => {
            let wa = weighted_degree(a, weights);
            let wb = weighted_degree(b, weights);
            match wa.cmp(&wb) {
                Ordering::Equal => Ok(cmp_lex(a, b)),
                other => Ok(other),
            }
        }
        MonomialOrder::Block { blocks } => {
            let n = a.len();
            let w = n / blocks.len();
            for (i, block_order) in blocks.iter().enumerate() {
                let start = i * w;
                let end = start + w;
                let ord = cmp_exponents_inner(&a[start..end], &b[start..end], block_order)?;
                if ord != Ordering::Equal {
                    return Ok(ord);
                }
            }
            Ok(Ordering::Equal)
        }
        MonomialOrder::Elimination { eliminate, rest } => {
            let e = *eliminate as usize;
            let ord_front = cmp_lex(&a[..e], &b[..e]);
            if ord_front != Ordering::Equal {
                return Ok(ord_front);
            }
            cmp_exponents_inner(&a[e..], &b[e..], rest)
        }
    }
}

fn total_degree(v: &[u32]) -> u64 {
    v.iter().map(|&e| u64::from(e)).sum()
}

fn weighted_degree(v: &[u32], weights: &[u32]) -> u64 {
    v.iter().zip(weights.iter()).map(|(&e, &w)| u64::from(e) * u64::from(w)).sum()
}

fn cmp_lex(a: &[u32], b: &[u32]) -> Ordering {
    for (&av, &bv) in a.iter().zip(b.iter()) {
        match av.cmp(&bv) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

fn cmp_grevlex(a: &[u32], b: &[u32]) -> Ordering {
    for (&av, &bv) in a.iter().zip(b.iter()).rev() {
        match av.cmp(&bv) {
            Ordering::Equal => {}
            other => return other.reverse(),
        }
    }
    Ordering::Equal
}

fn order_invalid(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::PolynomialOrderInvalid).detail("domain", "polynomial").detail("operation", reason)
}
