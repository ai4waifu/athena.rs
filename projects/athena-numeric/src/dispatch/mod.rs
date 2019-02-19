//! 执行策略：宽度分派 executor · 能力三分 · 宿主上报门面。
//!
//! **不是** machine kernel。Machine kernel 在 [`crate::kernel`]。
//! 能力三分见 [`capability`]；宽度分派见 [`executor`]。

mod backend;
mod capability;
mod executor;

pub use backend::PortableBackend;
pub(crate) use backend::PORTABLE_WIRE_PAYLOAD_LIMIT_BYTES;
pub use capability::{AlgorithmCapability, CapabilityBundle, MachineCapability, ResourceCapability};
pub use executor::NumericExecutor;

use crate::representation::{domain::NumericDomain, precision::PrecisionKind};

/// 宿主域能力广告（过渡门面；与 Machine/Algorithm/Resource 正交，勿再往此枚举塞 ISA）。
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

/// 宿主上报用的数值运算标签（过渡门面；非 kernel 入口）。
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

/// 宿主上报用的结果语义标签（过渡门面）。
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

/// 资源与 wire 上限（ResourceCapability 过渡面）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericBackendLimits {
    /// 大整数最大 limb 数（`None` = 合同无上界）。
    pub max_limbs: Option<u32>,
    /// 任意精度实数最大尾数位数。
    pub max_significand_bits: Option<u32>,
    /// 规范 wire 幅度 / 域载荷解码的最大二进制字节数。
    pub max_wire_payload_bytes: Option<u32>,
    /// `pow` 指数幅度上限（`None` = 默认策略）。
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

/// 静态资源/宿主合同（过渡；非 [`crate::kernel`] 的 `KernelTable`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericBackendContract {
    /// 稳定 id。
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

/// 宿主能力面（过渡门面；machine kernel 入口是 `*_into` / KernelTable）。
pub trait NumericBackend {
    /// 完整静态合同。
    fn contract(&self) -> &'static NumericBackendContract;

    /// 名称。
    fn name(&self) -> &'static str {
        self.contract().id
    }

    /// 是否可用于 wasm32。
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
