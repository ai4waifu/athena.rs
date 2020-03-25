//! 单项式布局与环 intern 时编译的单项式序比较器。
//!
//! 算法热路径使用 [`MonomialLayout::cmp_exponents_desc`] 与 [`PackedMonomial`]，
//! 不再递归解释 [`super::order::MonomialOrder`]。

use std::cmp::Ordering;

use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::{exponent::add_exponent_vectors, order::MonomialOrder};

/// 环 intern 时编译的单项式序（内循环 infallible 比较）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
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

impl CompiledMonomialOrder {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Lex { variables } => Self::Lex { variables: *variables },
            Self::GrLex { variables } => Self::GrLex { variables: *variables },
            Self::GrevLex { variables } => Self::GrevLex { variables: *variables },
            Self::Weighted { weights } => Self::Weighted { weights: weights.clone() },
            Self::Block { segments } => Self::Block { segments: segments.iter().map(CompiledBlockSegment::owning_copy).collect() },
            Self::Elimination { front, rest } => Self::Elimination { front: *front, rest: Box::new(rest.owning_copy()) },
        }
    }
}

/// 分块序的一段。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct CompiledBlockSegment {
    /// 段起始（含）。
    pub start: usize,
    /// 段结束（不含）。
    pub end: usize,
    /// 段内序。
    pub order: CompiledMonomialOrder,
}

impl CompiledBlockSegment {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { start: self.start, end: self.end, order: self.order.owning_copy() }
    }
}

/// packed word 单项式指数（arena 友好；比较经 layout 解码）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct PackedMonomial {
    words: Vec<u64>,
}

impl PackedMonomial {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { words: self.words.clone() }
    }

    /// 空 packed 单项式（全零指数）。
    pub fn zero(words: usize) -> Self {
        Self { words: vec![0u64; words] }
    }

    /// packed limb 切片（只读）。
    pub fn words(&self) -> &[u64] {
        &self.words
    }
}

/// 环上的单项式布局（intern 时固定）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct MonomialLayout {
    variable_count: usize,
    bits_per_exponent: u8,
    packed_words_per_monomial: usize,
    max_exponent: u32,
    compiled_order: CompiledMonomialOrder,
}

impl MonomialLayout {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            variable_count: self.variable_count,
            bits_per_exponent: self.bits_per_exponent,
            packed_words_per_monomial: self.packed_words_per_monomial,
            max_exponent: self.max_exponent,
            compiled_order: self.compiled_order.owning_copy(),
        }
    }

    /// 由声明式序与变量数编译布局（环构造时调用一次）。
    pub fn compile(order: &MonomialOrder, variable_count: usize) -> Result<Self> {
        order.validate_for_variables(variable_count)?;
        let bits_per_exponent = select_bits_per_exponent(variable_count);
        let max_exponent = max_exponent_for_bits(bits_per_exponent);
        let packed_words_per_monomial = packed_word_count(variable_count, bits_per_exponent);
        Ok(Self {
            variable_count,
            bits_per_exponent,
            packed_words_per_monomial,
            max_exponent,
            compiled_order: CompiledMonomialOrder::compile(order, variable_count)?,
        })
    }

    /// 变量数。
    pub fn variable_count(&self) -> usize {
        self.variable_count
    }

    /// 每指数占用位数。
    pub fn bits_per_exponent(&self) -> u8 {
        self.bits_per_exponent
    }

    /// 每个 packed 单项式占用的 `u64` word 数。
    pub fn packed_words_per_monomial(&self) -> usize {
        self.packed_words_per_monomial
    }

    /// 可编码的最大指数（含）。
    pub fn max_exponent(&self) -> u32 {
        self.max_exponent
    }

    /// 已编译序（只读）。
    pub fn compiled_order(&self) -> &CompiledMonomialOrder {
        &self.compiled_order
    }

    /// 将指数向量编码为 packed word（溢出返回 [`DiagnosticCode::PolynomialDegreeOverflow`]）。
    pub fn pack(&self, exponents: &[u32]) -> Result<PackedMonomial> {
        self.validate_exponents(exponents)?;
        for &e in exponents {
            if e > self.max_exponent {
                return Err(degree_overflow());
            }
        }
        Ok(PackedMonomial { words: pack_words(exponents, self.bits_per_exponent, self.packed_words_per_monomial) })
    }

    /// 解码 packed 单项式为指数向量。
    pub fn unpack<'a>(&self, packed: &'a PackedMonomial) -> Result<Vec<u32>> {
        if packed.words.len() != self.packed_words_per_monomial {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "packed_monomial_word_length"));
        }
        Ok(unpack_words(&packed.words, self.variable_count, self.bits_per_exponent))
    }

    /// 比较两个指数向量（升序语义：`a > b` 当 `a` 在单项式序中更大）。
    pub fn cmp_exponents(&self, a: &[u32], b: &[u32]) -> Ordering {
        debug_assert_eq!(a.len(), self.variable_count);
        debug_assert_eq!(b.len(), self.variable_count);
        self.compiled_order.cmp_exponents(a, b)
    }

    /// 比较两个 packed 单项式（与 [`Self::cmp_exponents`] 一致）。
    pub fn cmp_packed(&self, a: &PackedMonomial, b: &PackedMonomial) -> Result<Ordering> {
        let ae = self.unpack(a)?;
        let be = self.unpack(b)?;
        Ok(self.cmp_exponents(&ae, &be))
    }

    /// 降序比较（leading term 在前；canonical 排序用）。
    pub fn cmp_exponents_desc(&self, a: &[u32], b: &[u32]) -> Ordering {
        self.cmp_exponents(b, a)
    }

    /// packed 降序比较。
    pub fn cmp_packed_desc(&self, a: &PackedMonomial, b: &PackedMonomial) -> Result<Ordering> {
        Ok(self.cmp_packed(a, b)?.reverse())
    }

    /// 指数向量是否相等。
    pub fn exponents_equal(&self, a: &[u32], b: &[u32]) -> bool {
        a.len() == self.variable_count && b.len() == self.variable_count && a == b
    }

    /// packed 单项式是否相等。
    pub fn packed_equal(&self, a: &PackedMonomial, b: &PackedMonomial) -> bool {
        a.words == b.words
    }

    /// 单项式整除：`divisor | target`。
    pub fn monomial_divides(&self, divisor: &[u32], target: &[u32]) -> bool {
        debug_assert_eq!(divisor.len(), self.variable_count);
        debug_assert_eq!(target.len(), self.variable_count);
        divisor.iter().zip(target.iter()).all(|(&d, &t)| d <= t)
    }

    /// packed 整除判定。
    pub fn packed_divides(&self, divisor: &PackedMonomial, target: &PackedMonomial) -> Result<bool> {
        let d = self.unpack(divisor)?;
        let t = self.unpack(target)?;
        Ok(self.monomial_divides(&d, &t))
    }

    /// 最小公倍单项式指数。
    pub fn lcm_exponents(&self, a: &[u32], b: &[u32]) -> Result<Vec<u32>> {
        self.validate_exponents(a)?;
        self.validate_exponents(b)?;
        Ok(a.iter().zip(b.iter()).map(|(&x, &y)| x.max(y)).collect())
    }

    /// lcm 并返回 packed 形式。
    pub fn lcm_packed(&self, a: &PackedMonomial, b: &PackedMonomial) -> Result<PackedMonomial> {
        let lcm = self.lcm_exponents(&self.unpack(a)?, &self.unpack(b)?)?;
        self.pack(&lcm)
    }

    /// `num - den`（逐分量；Gröbner S-pair 用）。
    pub fn exponents_delta(&self, num: &[u32], den: &[u32]) -> Result<Vec<u32>> {
        self.validate_exponents(num)?;
        self.validate_exponents(den)?;
        num.iter().zip(den.iter()).map(|(&n, &d)| n.checked_sub(d).ok_or_else(degree_overflow)).collect()
    }

    /// 与 [`super::exponent::add_exponent_vectors`] 相同语义，经 layout 校验长度。
    pub fn add_exponents(&self, a: &[u32], b: &[u32]) -> Result<Vec<u32>> {
        self.validate_exponents(a)?;
        self.validate_exponents(b)?;
        add_exponent_vectors(a, b)
    }

    /// 校验指数向量长度与布局一致。
    pub fn validate_exponents(&self, exponents: &[u32]) -> Result<()> {
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
    pub fn compile(order: &MonomialOrder, variable_count: usize) -> Result<Self> {
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
                    segments.push(CompiledBlockSegment { start, end, order: Self::compile(block_order, width)? });
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

fn select_bits_per_exponent(variable_count: usize) -> u8 {
    if variable_count <= 8 { 16 } else { 32 }
}

fn max_exponent_for_bits(bits: u8) -> u32 {
    match bits {
        16 => u16::MAX as u32,
        _ => u32::MAX,
    }
}

fn packed_word_count(variable_count: usize, bits_per_exponent: u8) -> usize {
    let total_bits = variable_count * bits_per_exponent as usize;
    total_bits.div_ceil(64)
}

fn pack_words(exponents: &[u32], bits: u8, word_count: usize) -> Vec<u64> {
    let mut out = vec![0u64; word_count];
    let mask = if bits == 16 { 0xffffu64 } else { 0xffff_ffffu64 };
    for (i, &e) in exponents.iter().enumerate() {
        let bit_offset = i * bits as usize;
        let word_idx = bit_offset / 64;
        let shift = bit_offset % 64;
        let v = u64::from(e) & mask;
        if shift + bits as usize <= 64 {
            out[word_idx] |= v << shift;
        }
        else {
            let low_bits = 64 - shift;
            out[word_idx] |= v << shift;
            out[word_idx + 1] |= v >> low_bits;
        }
    }
    out
}

fn unpack_words(words: &[u64], variable_count: usize, bits: u8) -> Vec<u32> {
    let mask = if bits == 16 { 0xffffu64 } else { 0xffff_ffffu64 };
    let mut out = Vec::with_capacity(variable_count);
    for i in 0..variable_count {
        let bit_offset = i * bits as usize;
        let word_idx = bit_offset / 64;
        let shift = bit_offset % 64;
        let v = if shift + bits as usize <= 64 {
            (words[word_idx] >> shift) & mask
        }
        else {
            let low_bits = 64 - shift;
            let lo = words[word_idx] >> shift;
            let hi = words.get(word_idx + 1).copied().unwrap_or(0) << low_bits;
            (lo | hi) & mask
        };
        out.push(v as u32);
    }
    out
}

fn degree_overflow() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::PolynomialDegreeOverflow).detail("domain", "polynomial").detail("operation", "packed_exponent_overflow")
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
