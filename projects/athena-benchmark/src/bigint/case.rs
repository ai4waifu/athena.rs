//! bigint 对照用例维度。

use serde::Serialize;

/// 位宽矩阵。
pub const BITS: &[u32] = &[64, 256, 1024, 4096];

/// 算术操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BigIntOp {
    /// 加法。
    Add,
    /// 乘法。
    Mul,
    /// 除法（`prod / a`，商为 `b`）。
    Div,
    /// 最大公约数。
    Gcd,
    /// 幂（小原生指数）。
    Pow,
}

impl BigIntOp {
    /// 全部操作。
    pub const ALL: &[Self] = &[Self::Add, Self::Mul, Self::Div, Self::Gcd, Self::Pow];

    /// 稳定短名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Div => "div",
            Self::Gcd => "gcd",
            Self::Pow => "pow",
        }
    }
}

/// 测量层次（禁止互相冒充）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchLayer {
    /// Limb / `Natural`：`GcMode::Disabled`，复用 context。
    Kernel,
    /// `Integer` + 复用 `NumericContext`：`GcMode::Deferred`。
    Numeric,
    /// 公共便利入口（每次隐式建 context）：`GcMode::Auto`。
    E2e,
    /// 外部对照库。
    Peer,
}

impl BenchLayer {
    /// 稳定短名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Numeric => "numeric",
            Self::E2e => "e2e",
            Self::Peer => "peer",
        }
    }

    /// 建议的 `GcMode` 报告名（peer 为 `n/a`）。
    pub fn suggested_gc_mode(self) -> &'static str {
        match self {
            Self::Kernel => "disabled",
            Self::Numeric => "deferred",
            Self::E2e => "auto",
            Self::Peer => "n/a",
        }
    }
}

/// Context 生命周期策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPolicy {
    /// 迭代外构造一次，热路径 `try_*`。
    Reused,
    /// 每次公共 API 自行 `pure_rust_default()`。
    PerCall,
}

impl ContextPolicy {
    /// 稳定短名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reused => "reused",
            Self::PerCall => "per_call",
        }
    }
}

/// 实现方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Implementation {
    /// Athena。
    Athena,
    /// `num-bigint`。
    Num,
    /// `ibig`。
    Ibig,
    /// `malachite`。
    Malachite,
}

impl Implementation {
    /// 稳定短名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Athena => "athena",
            Self::Num => "num",
            Self::Ibig => "ibig",
            Self::Malachite => "malachite",
        }
    }

    /// 是否需要对应 optional feature。
    pub fn feature_enabled(self) -> bool {
        match self {
            Self::Athena => true,
            Self::Num => cfg!(feature = "compare-num-bigint"),
            Self::Ibig => cfg!(feature = "compare-ibig"),
            Self::Malachite => cfg!(feature = "compare-malachite"),
        }
    }
}

/// 一条统一对照用例。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BenchCase {
    /// 操作。
    pub operation: BigIntOp,
    /// 位宽。
    pub bits: u32,
    /// 实现。
    pub implementation: Implementation,
    /// 测量层。
    pub layer: BenchLayer,
    /// Context 策略。
    pub context_policy: ContextPolicy,
}

impl BenchCase {
    /// 稳定 id：`bigint.<op>.<bits>.<impl>.<layer>`。
    pub fn id(self) -> String {
        format!(
            "bigint.{}.{}.{}.{}",
            self.operation.as_str(),
            self.bits,
            self.implementation.as_str(),
            self.layer.as_str()
        )
    }

    /// Criterion `BenchmarkId` 函数名段：`<layer>/<impl>`。
    pub fn criterion_function(self) -> String {
        format!("{}/{}", self.layer.as_str(), self.implementation.as_str())
    }

    /// 建议 gc mode 报告字段。
    pub fn gc_mode(self) -> &'static str {
        self.layer.suggested_gc_mode()
    }
}

/// 完整矩阵（仅包含当前 feature 启用的实现）。
pub fn all_cases() -> Vec<BenchCase> {
    let mut out = Vec::new();
    for &op in BigIntOp::ALL {
        out.extend(cases_for_op(op));
    }
    out
}

/// 单个操作的矩阵行。
pub fn cases_for_op(operation: BigIntOp) -> Vec<BenchCase> {
    let mut out = Vec::new();
    for &bits in BITS {
        out.push(BenchCase {
            operation,
            bits,
            implementation: Implementation::Athena,
            layer: BenchLayer::Kernel,
            context_policy: ContextPolicy::Reused,
        });
        out.push(BenchCase {
            operation,
            bits,
            implementation: Implementation::Athena,
            layer: BenchLayer::Numeric,
            context_policy: ContextPolicy::Reused,
        });
        out.push(BenchCase {
            operation,
            bits,
            implementation: Implementation::Athena,
            layer: BenchLayer::E2e,
            context_policy: ContextPolicy::PerCall,
        });

        for implementation in [Implementation::Num, Implementation::Ibig, Implementation::Malachite] {
            if !implementation.feature_enabled() {
                continue;
            }
            out.push(BenchCase {
                operation,
                bits,
                implementation,
                layer: BenchLayer::Peer,
                context_policy: ContextPolicy::Reused,
            });
        }
    }
    out
}
