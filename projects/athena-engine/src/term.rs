//! 过渡求值用的运行时表达式树 [`Term`]（非 arena IR）。

use std::fmt;

use athena_numeric::{Number, to_f64_lossy as num_to_f64_lossy};

/// 从项原子中提取内核数字。
pub fn number_from_term(term: &Term) -> Option<&Number> {
    match term {
        Term::Atom(Atom::Number(n)) => Some(n),
        _ => None,
    }
}

/// 引擎 IR 中的原子值。
#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    /// 统一内核数字（唯一数值真相源：[`Number`] = [`athena_numeric::NumericValue`]）。
    Number(Number),
    /// 字符串值（已由宿主 / 方言层解码）。
    String(String),
    /// 符号名。
    Symbol(String),
}

/// 过渡求值用的运行时表达式树（非方言 AST，非 arena IR）。
#[derive(Debug, Clone, PartialEq)]
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
    /// 符号原子。
    pub fn symbol(name: impl Into<String>) -> Self {
        Self::Atom(Atom::Symbol(name.into()))
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

    /// 是否为数值 `-1`。
    pub fn is_neg_one(&self) -> bool {
        self.as_number().is_some_and(Number::is_neg_one)
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
