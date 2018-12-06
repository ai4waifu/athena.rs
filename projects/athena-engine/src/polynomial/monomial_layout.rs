//! 单项式布局与环 intern 时编译的单项式序比较器。
//!
//! 算法热路径使用 [`MonomialLayout::cmp_exponents_desc`]，不再递归解释 [`super::order::MonomialOrder`]。

use std::cmp::Ordering;

use athena_types::{Diagnostic, DiagnosticCode};

use super::order::MonomialOrder;

/// 环 intern 时编译的单项式序（内循环 infallible 比较）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledMonomialOrder {
    /// 字典序。
    Lex {
        /// 变量数。
        variables: usize,
    },
    /// 先总次数，再字典序。
    GrLex {
        /// 变量数。
        variables: usize,
    },
    /// 先总次数，再逆字典序。
    GrevLex {
        /// 变量数。
        variables: usize,
    },
    /// 加权次数，再字典序破 tie。
    Weighted {
        /// 与变量表等长权重。
        weights: Vec<u32>,
    },
    /// 分块序（每段显式范围 + 子序）。
    Block {
        /// 有序段。
        segments: Vec<CompiledBlockSegment>,
    },
    /// 消元序。
    Elimination {
        /// 消元块宽度。
        front: usize,
        /// 剩余变量序。
        rest: Box<CompiledMonomialOrder>,
    },
}

/// 分块序的一段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledBlockSegment {
    /// 段起始（含）。
    pub start: usize,
    /// 段结束（不含）。
    pub end: usize,
    /// 段内序。
    pub order: CompiledMonomialOrder,
}

/// 环上的单项式布局（intern 时固定；后续可扩展 packed word 存储）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonomialLayout {
    variable_count: usize,
    bits_per_exponent: u8,
    compiled_order: CompiledMonomialOrder,
}

impl MonomialLayout {
    /// 由声明式序与变量数编译布局（环构造时调用一次）。
    pub fn compile(order: &MonomialOrder, variable_count: usize) -> Result<Self, Diagnostic> {
        order.validate_for_variables(variable_count)?;
        Ok(Self {
            variable_count,
            bits_per_exponent: 32,
            compiled_order: CompiledMonomialOrder::compile(order, variable_count)?,
        })
    }

    /// 变量数。
    pub fn variable_count(&self) -> usize {
        self.variable_count
    }

    /// 每指数占用位数（当前固定 32；packed path 预留）。
    pub fn bits_per_exponent(&self) -> u8 {
        self.bits_per_exponent
    }

    /// 已编译序（只读）。
    pub fn compiled_order(&self) -> &CompiledMonomialOrder {
        &self.compiled_order
    }

    /// 比较两个指数向量（升序语义：`a > b` 当 `a` 在单项式序中更大）。
    pub fn cmp_exponents(&self, a: &[u32], b: &[u32]) -> Ordering {
        debug_assert_eq!(a.len(), self.variable_count);
        debug_assert_eq!(b.len(), self.variable_count);
        self.compiled_order.cmp_exponents(a, b)
    }

    /// 降序比较（leading term 在前；canonical 排序用）。
    pub fn cmp_exponents_desc(&self, a: &[u32], b: &[u32]) -> Ordering {
        self.cmp_exponents(b, a)
    }

    /// 校验指数向量长度与布局一致。
    pub fn validate_exponents(&self, exponents: &[u32]) -> Result<(), Diagnostic> {
        if exponents.len() != self.variable_count {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "monomial_layout_exponent_length"));
        }
        Ok(())
    }
}

impl CompiledMonomialOrder {
    /// 由声明式序编译（递归展开 Block / Elimination）。
    pub fn compile(order: &MonomialOrder, variable_count: usize) -> Result<Self, Diagnostic> {
        match order {
            MonomialOrder::Lex => Ok(Self::Lex { variables: variable_count }),
            MonomialOrder::GrLex => Ok(Self::GrLex { variables: variable_count }),
            MonomialOrder::GrevLex => Ok(Self::GrevLex { variables: variable_count }),
            MonomialOrder::Weighted { weights } => Ok(Self::Weighted { weights: weights.clone() }),
            MonomialOrder::Block { blocks } => {
                let width = variable_count / blocks.len();
                let mut segments = Vec::with_capacity(blocks.len());
                for (i, block_order) in blocks.iter().enumerate() {
                    let start = i * width;
                    let end = start + width;
                    segments.push(CompiledBlockSegment {
                        start,
                        end,
                        order: Self::compile(block_order, width)?,
                    });
                }
                Ok(Self::Block { segments })
            }
            MonomialOrder::Elimination { eliminate, rest } => {
                let front = *eliminate as usize;
                let rest_n = variable_count.saturating_sub(front);
                Ok(Self::Elimination { front, rest: Box::new(Self::compile(rest, rest_n)?) })
            }
        }
    }

    /// infallible 指数比较（编译时已校验变量数）。
    pub fn cmp_exponents(&self, a: &[u32], b: &[u32]) -> Ordering {
        match self {
            Self::Lex { .. } => cmp_lex(a, b),
            Self::GrLex { .. } => {
                let da = total_degree(a);
                let db = total_degree(b);
                match da.cmp(&db) {
                    Ordering::Equal => cmp_lex(a, b),
                    other => other,
                }
            }
            Self::GrevLex { .. } => {
                let da = total_degree(a);
                let db = total_degree(b);
                match da.cmp(&db) {
                    Ordering::Equal => cmp_grevlex(a, b),
                    other => other,
                }
            }
            Self::Weighted { weights } => {
                let wa = weighted_degree(a, weights);
                let wb = weighted_degree(b, weights);
                match wa.cmp(&wb) {
                    Ordering::Equal => cmp_lex(a, b),
                    other => other,
                }
            }
            Self::Block { segments } => {
                for seg in segments {
                    let ord = seg.order.cmp_exponents(&a[seg.start..seg.end], &b[seg.start..seg.end]);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
                Ordering::Equal
            }
            Self::Elimination { front, rest } => {
                let ord_front = cmp_lex(&a[..*front], &b[..*front]);
                if ord_front != Ordering::Equal {
                    return ord_front;
                }
                rest.cmp_exponents(&a[*front..], &b[*front..])
            }
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
