//! 能力三分：Machine / Algorithm / Resource（禁止糊进单一 `NumericCapability`）。

use super::NumericBackendLimits;

/// 机器指令集能力（context 创建时冻结，热路径不再 detect）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MachineCapability {
    /// x86 BMI2/ADX 风格进位链。
    pub adx: bool,
    /// BMI2。
    pub bmi2: bool,
    /// AVX2（预留）。
    pub avx2: bool,
    /// AArch64 进位指令路径。
    pub aarch64_carry: bool,
    /// wasm SIMD 路径。
    pub wasm_simd: bool,
}

impl MachineCapability {
    /// portable 语义基线（无 ISA 特化）。
    pub const PORTABLE: Self = Self { adx: false, bmi2: false, avx2: false, aarch64_carry: false, wasm_simd: false };

    /// 本构建目标上可声明的 ISA 能力（编译期静态，非热路径 detect）。
    pub fn detect_host() -> Self {
        #[allow(unused_mut)]
        let mut m = Self::PORTABLE;
        #[cfg(all(target_arch = "x86_64", target_feature = "adx"))]
        {
            m.adx = true;
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
        {
            m.bmi2 = true;
        }
        #[cfg(target_arch = "aarch64")]
        {
            m.aarch64_carry = true;
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            m.wasm_simd = true;
        }
        m
    }
}

/// 算法策略能力（由 planner 选择，不进 machine kernel）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlgorithmCapability {
    /// Schoolbook 乘 / 除。
    pub schoolbook: bool,
    /// Karatsuba 乘。
    pub karatsuba: bool,
    /// Toom 宽乘（三路分块路径已启用）。
    pub toom: bool,
    /// Burnikel–Ziegler 除法。
    pub bz_division: bool,
    /// Half-GCD（预留）。
    pub half_gcd: bool,
    /// Montgomery 模幂。
    pub montgomery: bool,
}

impl Default for AlgorithmCapability {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl AlgorithmCapability {
    /// 默认 pure-Rust 算法面（含 Toom-3 / BZ 门控路径）。
    pub const DEFAULT: Self =
        Self { schoolbook: true, karatsuba: true, toom: true, bz_division: true, half_gcd: false, montgomery: true };
}

/// 资源能力（预算 / scratch / 目标复用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceCapability {
    /// 与 [`NumericBackendLimits`] 对齐的静态上限。
    pub limits: NumericBackendLimits,
    /// 是否允许复用 destination buffer。
    pub can_reuse_destination: bool,
    /// 常数时间路径（预留；默认关闭）。
    pub constant_time: bool,
}

impl Default for ResourceCapability {
    fn default() -> Self {
        Self { limits: NumericBackendLimits::default(), can_reuse_destination: true, constant_time: false }
    }
}

impl ResourceCapability {
    /// 由 backend limits 构造。
    pub fn from_limits(limits: NumericBackendLimits) -> Self {
        Self { limits, can_reuse_destination: true, constant_time: false }
    }

    /// 无上限（测试）。
    pub fn unlimited() -> Self {
        Self {
            limits: NumericBackendLimits {
                max_limbs: None,
                max_significand_bits: None,
                max_wire_payload_bytes: None,
                max_pow_exp: None,
            },
            can_reuse_destination: true,
            constant_time: false,
        }
    }
}

/// Context 级冻结的能力束（创建时绑定，热路径只读）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityBundle {
    /// 机器层。
    pub machine: MachineCapability,
    /// 算法层。
    pub algorithm: AlgorithmCapability,
    /// 资源层。
    pub resource: ResourceCapability,
}

impl CapabilityBundle {
    /// portable 默认束。
    pub fn portable_default() -> Self {
        Self {
            machine: MachineCapability::PORTABLE,
            algorithm: AlgorithmCapability::DEFAULT,
            resource: ResourceCapability::from_limits(NumericBackendLimits::default()),
        }
    }

    /// 主机 ISA + 默认算法/资源（仍在 context 创建时调用一次）。
    pub fn host_default() -> Self {
        Self {
            machine: MachineCapability::detect_host(),
            algorithm: AlgorithmCapability::DEFAULT,
            resource: ResourceCapability::from_limits(NumericBackendLimits::default()),
        }
    }
}
