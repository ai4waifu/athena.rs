//! 数论结果对象（非裸整数列表）。

use athena_numeric::{Integer, ModularValue, Modulus, Rational};

use super::certificates::{CompositeWitness, PrimeCertificate, ProbablePrimeEvidence};

/// Miller–Rabin 基选择策略。固定基可复现，但**不是**独立随机样本，
/// 不得按通常随机见证假设计算误判概率上界。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MillerRabinBaseSelection {
    /// 固定确定性序列（截断自引擎内建小素数表）。
    Fixed,
}

/// 素性判定结果 — 禁止把 Miller-Rabin probable 写成确定 `true`。
#[derive(Debug, PartialEq, Eq)]
pub enum Primality {
    /// 确定素数。
    Prime {
        /// 可独立核对的证书（当前为确定性测试路径描述）。
        certificate: PrimeCertificate,
    },
    /// 确定合数。
    Composite {
        /// 可验证见证。
        witness: CompositeWitness,
    },
    /// 概率素数。
    ProbablePrime {
        /// 实际执行证据。
        evidence: ProbablePrimeEvidence,
    },
    /// 未判定（例如请求 0 轮且无确定性路径）。
    Unknown,
}

impl Primality {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Prime { certificate } => Self::Prime { certificate: certificate.clone() },
            Self::Composite { witness } => Self::Composite { witness: witness.clone() },
            Self::ProbablePrime { evidence } => Self::ProbablePrime { evidence: evidence.clone() },
            Self::Unknown => Self::Unknown,
        }
    }
}

impl Clone for Primality {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}

/// 单个分解因子（底 × 指数）及其素性状态。
#[derive(Debug, PartialEq, Eq)]
pub struct FactorComponent {
    /// 底数（`> 1`）。
    pub base: Integer,
    /// 指数（`> 0`）。
    pub exponent: u32,
    /// 该底的素性状态。
    pub status: FactorBaseStatus,
}

/// 因子底的素性状态。
#[derive(Debug, PartialEq, Eq)]
pub enum FactorBaseStatus {
    /// 确定素数底。
    ProvenPrime {
        /// 证书。
        certificate: PrimeCertificate,
    },
    /// 概率素数底。
    ProbablePrime {
        /// 证据。
        evidence: ProbablePrimeEvidence,
    },
}

/// 余因子（cofactor）状态。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CofactorStatus {
    /// 完全分解，余因子为 1。
    One,
    /// 余因子为未继续分解的合数。
    CompositeUnsplit,
    /// 素性未决。
    Unknown,
}

/// 整数分解完整性（由 [`Factorization::completeness`] 从组件推导）。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FactorizationCompleteness {
    /// 完全分解为确定素因子，余因子为 1。
    Complete,
    /// 余因子为 1，但存在概率素因子。
    Probable,
    /// 仍有合数余因子。
    Partial,
    /// 触及资源 / 输入拒绝上限。
    ResourceLimited,
}

/// 带逐因子证据的整数分解。
///
/// 不变量（非零输入）：`input = unit * Π base^e * cofactor`，`unit ∈ {-1,1}`，
/// `base > 1`，`e > 0`。`0` 不进入本结构。
#[derive(Debug, PartialEq, Eq)]
pub struct Factorization {
    /// 单位（符号：`±1`）。
    pub unit: Integer,
    /// 因子（底升序）。
    pub factors: Vec<FactorComponent>,
    /// 未完全分解的余因子（完全分解时为 1）。
    pub cofactor: Integer,
    /// 余因子状态。
    pub cofactor_status: CofactorStatus,
    /// 是否因输入比特上限被拒绝（非已消耗预算）。
    pub input_rejected: bool,
    /// 是否因算法步数 / 预算耗尽而停止（可续算）。
    pub resource_exhausted: bool,
}

impl Factorization {
    /// 由组件与余因子状态推导整体完整性（不单独存储可能矛盾的 enum）。
    pub fn completeness(&self) -> FactorizationCompleteness {
        if self.input_rejected || self.resource_exhausted {
            return FactorizationCompleteness::ResourceLimited;
        }
        let has_probable = self.factors.iter().any(|c| matches!(c.status, FactorBaseStatus::ProbablePrime { .. }));
        let all_proven = self.factors.iter().all(|c| matches!(c.status, FactorBaseStatus::ProvenPrime { .. }));
        match self.cofactor_status {
            CofactorStatus::One if all_proven && !has_probable => FactorizationCompleteness::Complete,
            CofactorStatus::One if has_probable => FactorizationCompleteness::Probable,
            CofactorStatus::CompositeUnsplit | CofactorStatus::Unknown => FactorizationCompleteness::Partial,
            CofactorStatus::One => FactorizationCompleteness::Partial,
        }
    }

    /// 兼容旧字段名：未分解余因子。
    pub fn remainder(&self) -> &Integer {
        &self.cofactor
    }
}

/// extended Euclidean：`s·a + t·b = g`。
#[derive(Debug, PartialEq, Eq)]
pub struct ExtendedGcd {
    /// `gcd(|a|,|b|)`（非负）。
    pub g: Integer,
    /// Bézout `s`。
    pub s: Integer,
    /// Bézout `t`。
    pub t: Integer,
}

/// 数论域值。
#[derive(Debug, PartialEq, Eq)]
pub enum NumberTheoryValue {
    /// 整数标量（gcd / lcm 等）。
    Integer(Integer),
    /// 扩展 gcd。
    ExtendedGcd(ExtendedGcd),
    /// 素性。
    Primality(Primality),
    /// 分解。
    Factorization(Factorization),
    /// 模运算结果。
    Modular(ModularValue),
    /// 批量模逆结果（同一 [`ModulusId`]）。
    ModularList(Vec<ModularValue>),
    /// 线性同余解集。
    Congruence(CongruenceSolution),
    /// 中国剩余定理结果。
    Crt(CrtResult),
    /// 有理重构。
    RationalReconstruction(RationalReconstruction),
    /// 完全幂 `base^exponent`（`exponent > 1`）。
    PerfectPower {
        /// 底。
        base: Integer,
        /// 指数。
        exponent: u32,
    },
    /// 整数列表（如筛法素数表）。
    IntegerList(Vec<Integer>),
}

/// 线性同余 `a x ≡ b (mod m)` 的解结构。
#[derive(Debug, PartialEq, Eq)]
pub enum CongruenceSolution {
    /// `g ∤ b`：无解。
    NoSolution {
        /// `gcd(a, m)`。
        gcd: Integer,
        /// 不一致见证：`b mod g ≠ 0`。
        residue_mod_gcd: Integer,
    },
    /// `g = 1`：模 `m` 下唯一剩余类。
    UniqueClass {
        /// 解 `x₀ (mod m)`。
        residue: ModularValue,
    },
    /// `g > 1` 且 `g | b`：模 `m` 下有 `g` 个解，压缩为模 `m/g` 的一个基本类。
    MultipleClasses {
        /// 基本解 `0 ≤ x₀ < m/g`。
        base_residue: Integer,
        /// `m/g`。
        reduced_modulus: Modulus,
        /// 原模 `m`。
        ambient_modulus: Modulus,
        /// 解的个数 `g`。
        multiplicity: Integer,
    },
}

/// 广义 CRT 结果（允许非互素模数）。
#[derive(Debug, PartialEq, Eq)]
pub enum CrtResult {
    /// 相容：解模 `lcm(m_i)`。
    Consistent {
        /// 解剩余类。
        solution: ModularValue,
        /// 最终模数（lcm）。
        modulus_lcm: Modulus,
    },
    /// 不相容。
    Inconsistent {
        /// 冲突左侧方程下标。
        left_index: usize,
        /// 冲突右侧方程下标。
        right_index: usize,
        /// `gcd(m_left, m_right)`。
        gcd: Integer,
        /// `a_left − a_right`（未约化）。
        residue_difference: Integer,
    },
}

/// 有理数重构结果。
#[derive(Debug, PartialEq, Eq)]
pub enum RationalReconstruction {
    /// 找到满足界条件的既约分数。
    Found {
        /// 重构分数。
        value: Rational,
    },
    /// 在给定界下无唯一（或无）解。
    NotFound {
        /// 原因标签。
        reason: RationalReconstructionFailure,
    },
}

/// 有理重构失败原因。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RationalReconstructionFailure {
    /// 模数过小或界非法。
    InvalidBounds,
    /// extended Euclidean路径未产生满足 `|n|≤N`、`|d|≤D` 的解。
    NoCandidate,
}

/// 由素性结果构造因子底状态。
pub(crate) fn factor_status_from_primality(p: &Primality) -> Option<FactorBaseStatus> {
    match p {
        Primality::Prime { certificate } => Some(FactorBaseStatus::ProvenPrime { certificate: certificate.clone() }),
        Primality::ProbablePrime { evidence } => Some(FactorBaseStatus::ProbablePrime { evidence: evidence.clone() }),
        Primality::Composite { .. } | Primality::Unknown => None,
    }
}
