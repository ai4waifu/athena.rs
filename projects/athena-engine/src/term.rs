//! Legacy compatibility expression tree [`Term`]（非 arena AthenaIR）。
//!
//! # Architecture freeze（Living `25`）
//!
//! - [`Term`] is a **short-lived compatibility bridge** for Feature Gap 联调 and old tests.
//! - **Do not** add new domain APIs, matrix/Solve/calculus semantics, or Session contracts on [`Term`].
//! - **Do not** use [`Term`] as the long-term GC / N-API identity carrier.
//! - New code must target AthenaIR identities (`ExprId` / `ValueId` / `ResultId` / `ProofRef`).
//! - Atom representation fixes (e.g. typed `Boolean` / `Null`) are allowed; expanding heads is not.
//! - Living `19`: no Rust [`Clone`] on [`Term`] / [`Number`]; use [`Term::try_clone_in`] / `clone_term`.

use std::fmt;

use athena_numeric::{Number, NumericContext, to_f64_lossy as num_to_f64_lossy};
use athena_types::Result;

/// 从项原子中提取内核数字。
pub fn number_from_term(term: &Term) -> Option<&Number> {
    match term {
        Term::Atom(Atom::Number(n)) => Some(n),
        _ => None,
    }
}

/// 引擎过渡树上的原子值（legacy `Term` 桥 · Living `25`）。
///
/// Living `19`：不实现 [`Clone`]（[`Number`] 无 `Clone`）。深复制用 [`Self::try_clone_in`]。
#[derive(Debug, PartialEq)]
pub enum Atom {
    /// 统一内核数字（唯一数值真相源：[`Number`] = [`athena_numeric::NumericValue`]）。
    Number(Number),
    /// 字符串值（已由宿主 / 方言层解码）。
    String(String),
    /// 符号名。
    Symbol(String),
    /// Typed Boolean（不得长期用 [`Self::Symbol`] `"True"`/`"False"` 冒充）。
    Boolean(bool),
    /// Typed Null（不得长期用 [`Self::Symbol`] `"Null"` 或空串冒充）。
    Null,
}

impl Atom {
    /// Owning 复制：数字经 [`Number::try_clone_in`]。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> Result<Self> {
        Ok(match self {
            Self::Number(n) => Self::Number(n.try_clone_in(ctx)?),
            Self::String(s) => Self::String(s.clone()),
            Self::Symbol(s) => Self::Symbol(s.clone()),
            Self::Boolean(b) => Self::Boolean(*b),
            Self::Null => Self::Null,
        })
    }
}

/// Legacy 过渡求值树（非方言 AST，非 arena AthenaIR）。
///
/// **Living `25`**：compatibility bridge only。禁止新领域 API / 新公共语义返回类型。
/// Living `19`：不实现 [`Clone`]。结构深复制用 [`Self::try_clone_in`]。
#[derive(Debug, PartialEq)]
pub enum Term {
    /// 原子。
    Atom(Atom),
    /// 有序集合。
    List(Vec<Term>),
    /// 应用 `head(args…)`。
    Application {
        /// 头部项（通常为符号）。
        head: Box<Term>,
        /// 参数。
        arguments: Vec<Term>,
    },
}

impl Term {
    /// Owning 深复制：数字挂 `ctx`，字符串 / 符号普通拷贝。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> Result<Self> {
        Ok(match self {
            Self::Atom(a) => Self::Atom(a.try_clone_in(ctx)?),
            Self::List(xs) => {
                let mut out = Vec::with_capacity(xs.len());
                for x in xs {
                    out.push(x.try_clone_in(ctx)?);
                }
                Self::List(out)
            }
            Self::Application { head, arguments } => {
                let mut args = Vec::with_capacity(arguments.len());
                for a in arguments {
                    args.push(a.try_clone_in(ctx)?);
                }
                Self::Application { head: Box::new(head.try_clone_in(ctx)?), arguments: args }
            }
        })
    }

    /// 符号原子。
    pub fn symbol(name: impl Into<String>) -> Self {
        Self::Atom(Atom::Symbol(name.into()))
    }

    /// Typed Boolean 原子。
    pub fn boolean(value: bool) -> Self {
        Self::Atom(Atom::Boolean(value))
    }

    /// Typed `Null` 原子。
    pub fn null() -> Self {
        Self::Atom(Atom::Null)
    }

    /// 小型精确整数便捷构造。
    pub fn int(n: i64) -> Self {
        Self::number(Number::small_int(n))
    }

    /// 精确整数（`i64` 范围；更大整数须由宿主经 wire 解码为 [`Number`]）。
    pub fn integer(n: i64) -> Self {
        Self::int(n)
    }

    /// 精确有理数（`i64` 分子分母）。
    pub fn rational_i64(num: i64, den: i64) -> crate::Result<Self> {
        Ok(Self::number(Number::rational_i64(num, den)?))
    }

    /// 由已解码的 `f64` 构造机器实数。
    pub fn real(n: f64) -> Self {
        Self::number(Number::machine(n))
    }

    /// 由已解码的 [`Number`] 构造数字原子。
    pub fn number(n: Number) -> Self {
        Self::Atom(Atom::Number(n))
    }

    /// 符号头部的应用 `head(args…)`。
    pub fn apply(head: impl Into<String>, args: Vec<Term>) -> Self {
        Self::Application { head: Box::new(Self::symbol(head)), arguments: args }
    }

    /// 头部符号名（若有）。
    pub fn head_name(&self) -> Option<&str> {
        match self {
            Self::Application { head, .. } => match head.as_ref() {
                Self::Atom(Atom::Symbol(s)) => Some(s.as_str()),
                _ => None,
            },
            Self::List(_) => Some("List"),
            Self::Atom(Atom::Symbol(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// 内核数字引用（无精度损失）。
    pub fn as_number(&self) -> Option<&Number> {
        number_from_term(self)
    }

    /// 有损 `f64` — 仅用于显示 / 宿主提示。
    pub fn as_f64_lossy(&self) -> Option<f64> {
        self.as_number().and_then(|n| num_to_f64_lossy(n))
    }

    /// 是否为给定符号。
    pub fn is_symbol(&self, name: &str) -> bool {
        matches!(self, Self::Atom(Atom::Symbol(s)) if s == name)
    }

    /// 是否为数字 `-1`。
    pub fn is_neg_one(&self) -> bool {
        self.as_number().is_some_and(Number::is_neg_one)
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
