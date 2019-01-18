//! 数值 backend 合同与分派。
//!
//! 目录布局：
//! ```text
//! backend/
//!   mod.rs           — trait、能力标志、资源上限
//!   pure_rust/       — 默认 WASM 安全内核（limb 算术 + limb kernel）
//! ```
//!
//! 未来可选 backend（如 `native-accelerated/`）作为同级目录存在；
//! Rust 模块名用下划线（`pure_rust`），因为标识符不能含 `-`。

#[path = "pure_rust/mod.rs"]
pub mod pure_rust;

pub use pure_rust::PureRustBackend;

#[cfg(feature = "native-accelerated")]
pub mod native;

use crate::representation::{domain::NumericDomain, precision::PrecisionKind};

/// 分派与宿主上报用的能力标志。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericCapability {
    /// 精确整数算术。
    ExactInteger,
    /// 精确有理算术。
    ExactRational,
    /// IEEE binary64 机器实数。
    MachineReal,
    /// 任意精度实数骨架（IEEE754 promotion 路径）。
    ArbitraryRealSkeleton,
    /// 模整数约化。
    ModularInteger,
    /// 区间包络骨架。
    IntervalEnclosure,
    /// 区间端点的定向舍入。
    DirectedRounding,
    /// 带显式诊断的 promotion。
    ExplicitPromotion,
    /// 确定性纯 Rust 执行。
    Deterministic,
}

/// backend 可声明并分派的数值运算。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericOperation {
    /// 加法。
    Add,
    /// 减法。
    Sub,
    /// 乘法。
    Mul,
    /// 除法。
    Div,
    /// 幂运算。
    Pow,
    /// 取负。
    Neg,
    /// 绝对值。
    Abs,
    /// 平方根。
    Sqrt,
    /// 阶乘。
    Factorial,
    /// 最大公约数。
    Gcd,
    /// 有序比较。
    Compare,
    /// 域 / 精度 promotion。
    Promote,
    /// 区间加法。
    IntervalAdd,
    /// 区间乘法。
    IntervalMul,
}

/// backend 对某运算保证的结果语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericResultMode {
    /// 精确符号 / 整数 / 有理结果。
    Exact,
    /// IEEE binary64 机器结果。
    Machine,
    /// 任意精度实数骨架（IEEE754 位模式）。
    ArbitrarySkeleton,
    /// 带定向舍入的区间包络。
    IntervalEnclosure,
    /// 附带证明元数据的认证结果。
    Certified,
}

/// backend 声明的资源与 wire 上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericBackendLimits {
    /// 大整数最大 limb 数（`None` = 合同无上界）。
    pub max_limbs: Option<u32>,
    /// 任意精度实数最大尾数位数。
    pub max_significand_bits: Option<u32>,
    /// 规范 wire 幅度 / 域载荷解码的最大二进制字节数。
    pub max_wire_payload_bytes: Option<u32>,
    /// `pow` 指数幅度上限（`None` = backend 默认策略）。
    pub max_pow_exp: Option<i64>,
}

impl Default for NumericBackendLimits {
    fn default() -> Self {
        Self {
            max_limbs: None,
            max_significand_bits: Some(53),
            max_wire_payload_bytes: Some(1 << 20),
            max_pow_exp: Some(10_000),
        }
    }
}

/// 静态 backend 合同（能力、上限、可用性）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericBackendContract {
    /// 稳定 backend id。
    pub id: &'static str,
    /// 可用于 wasm32 构建。
    pub wasm_safe: bool,
    /// 需要仅原生库（BLAS/MKL/MPFR 候选）。
    pub native_only: bool,
    /// 可参与 JIT 编译计划。
    pub jit_eligible: bool,
    /// 固定策略下结果可复现。
    pub deterministic: bool,
    /// 规范二进制 wire（十进制文本仅经 [`crate::wire_text`] 显式使用）。
    pub default_radix: u8,
    /// 声明的能力。
    pub capabilities: &'static [NumericCapability],
    /// 资源上限。
    pub limits: NumericBackendLimits,
}

/// 数值 backend 能力面。
pub trait NumericBackend {
    /// 完整静态合同。
    fn contract(&self) -> &'static NumericBackendContract;

    /// backend 名称。
    fn name(&self) -> &'static str {
        self.contract().id
    }

    /// backend 是否可用于 wasm32。
    fn wasm_safe(&self) -> bool {
        self.contract().wasm_safe
    }

    /// 是否声明了某能力。
    fn has_capability(&self, cap: NumericCapability) -> bool {
        self.contract().capabilities.contains(&cap)
    }

    /// 当前成熟度门控下是否支持某域。
    fn supports_domain(&self, domain: &NumericDomain) -> bool;

    /// 是否支持某精度种类。
    fn supports_precision(&self, kind: PrecisionKind) -> bool;

    /// 是否能在 `domain` 上执行 `op` 并产出 `result`。
    fn supports_operation(&self, domain: &NumericDomain, op: NumericOperation, result: NumericResultMode) -> bool;
}

/// 默认纯 Rust backend 的 wire 载荷字节上限（与解码共用）。
pub(crate) use pure_rust::PURE_RUST_WIRE_PAYLOAD_LIMIT_BYTES;
