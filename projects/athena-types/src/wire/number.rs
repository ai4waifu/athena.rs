//! 数字类型定义（合同层 wire；不含求值算法与 `num-*`）。

/// 精确整数或既约有理数（十进制 wire，分母为正）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactNumber {
    /// 任意精度整数十进制（可含前导 `+/-`）。
    Integer(String),
    /// 既约有理数（分子 / 分母十进制）。
    Rational {
        /// 分子。
        numer: String,
        /// 分母（正）。
        denom: String,
    },
}

impl ExactNumber {
    /// 既约有理数（`i64` 分子 / 分母；在 types 层用 `i64` 归约）。
    pub fn rational_i64(num: i64, den: i64) -> Self {
        let mut n = num;
        let mut d = den;
        if d < 0 {
            n = -n;
            d = -d;
        }
        let g = gcd_i64(n.abs(), d.abs());
        n /= g;
        d /= g;
        if d == 1 { Self::Integer(n.to_string()) } else { Self::Rational { numer: n.to_string(), denom: d.to_string() } }
    }

    /// 渲染为字面量字符串。
    pub fn to_render_string(&self) -> String {
        match self {
            Self::Integer(s) => s.clone(),
            Self::Rational { numer, denom } => format!("{numer}/{denom}"),
        }
    }

    /// Whether exactly zero.
    pub fn is_zero(&self) -> bool {
        match self {
            Self::Integer(s) => decimal_is_zero(s),
            Self::Rational { numer, .. } => decimal_is_zero(numer),
        }
    }

    /// Whether exactly one.
    pub fn is_one(&self) -> bool {
        match self {
            Self::Integer(s) => decimal_as_i64(s) == Some(1),
            Self::Rational { numer, denom } => decimal_as_i64(numer) == Some(1) && decimal_as_i64(denom) == Some(1),
        }
    }

    /// Whether exactly `-1`.
    pub fn is_neg_one(&self) -> bool {
        match self {
            Self::Integer(s) => decimal_as_i64(s) == Some(-1),
            Self::Rational { numer, denom } => decimal_as_i64(numer) == Some(-1) && decimal_as_i64(denom) == Some(1),
        }
    }

    /// Integer when representable as `i64`.
    pub fn as_integer_exp(&self) -> Option<i64> {
        match self {
            Self::Integer(s) => decimal_as_i64(s),
            Self::Rational { numer, denom } if decimal_as_i64(denom) == Some(1) => decimal_as_i64(numer),
            _ => None,
        }
    }
}

/// Inexact real storage (phase 1: machine float only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RealNumber {
    /// IEEE-754 binary64.
    Machine(f64),
}

impl RealNumber {
    /// Whether zero.
    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Machine(n) if *n == 0.0)
    }

    /// Whether one.
    pub fn is_one(&self) -> bool {
        matches!(self, Self::Machine(n) if *n == 1.0)
    }

    /// Whether `-1`.
    pub fn is_neg_one(&self) -> bool {
        matches!(self, Self::Machine(n) if *n == -1.0)
    }
}

/// Unified kernel number (wire-stable representation).
#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    /// Exact integer or rational.
    Exact(ExactNumber),
    /// Inexact real.
    Real(RealNumber),
}

impl Number {
    /// Machine real.
    pub fn machine(n: f64) -> Self {
        Self::Real(RealNumber::Machine(n))
    }

    /// Small `i64` convenience.
    pub fn small_int(n: i64) -> Self {
        Self::Exact(ExactNumber::Integer(n.to_string()))
    }

    /// Parse decimal integer string (optional `+/-`).
    pub fn from_decimal_str(s: &str) -> Option<Self> {
        Self::from_exact_literal(s)
    }

    /// Parse exact literal: integer or `numer/denom` decimal wire (SXO / host frontend).
    pub fn from_exact_literal(s: &str) -> Option<Self> {
        let t = s.trim();
        if t.is_empty() {
            return None;
        }
        if let Some((numer, denom)) = t.split_once('/') {
            let numer = numer.trim();
            let denom = denom.trim();
            if !is_decimal_int_wire(numer) || !is_decimal_int_wire(denom) {
                return None;
            }
            let numer = normalize_int_wire(numer);
            let denom = normalize_int_wire(denom);
            if decimal_is_zero(&denom) {
                return None;
            }
            Some(Self::Exact(ExactNumber::Rational { numer, denom }))
        }
        else if is_decimal_int_wire(t) {
            Some(Self::Exact(ExactNumber::Integer(normalize_int_wire(t))))
        }
        else {
            None
        }
    }

    /// Exact rational from `i64` numerator / denominator.
    pub fn rational_i64(num: i64, den: i64) -> crate::Result<Self> {
        use crate::{Diagnostic, DiagnosticCode};
        if den == 0 {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero));
        }
        Ok(Self::Exact(ExactNumber::rational_i64(num, den)))
    }

    /// 渲染为字面量字符串。
    pub fn to_render_string(&self) -> String {
        match self {
            Self::Exact(e) => e.to_render_string(),
            Self::Real(RealNumber::Machine(n)) => format_machine(*n),
        }
    }

    /// Whether exactly zero.
    pub fn is_zero(&self) -> bool {
        match self {
            Self::Exact(e) => e.is_zero(),
            Self::Real(r) => r.is_zero(),
        }
    }

    /// Whether exactly one.
    pub fn is_one(&self) -> bool {
        match self {
            Self::Exact(e) => e.is_one(),
            Self::Real(r) => r.is_one(),
        }
    }

    /// Whether exactly `-1`.
    pub fn is_neg_one(&self) -> bool {
        match self {
            Self::Exact(e) => e.is_neg_one(),
            Self::Real(r) => r.is_neg_one(),
        }
    }

    /// Truthiness for logic (exact non-zero → true; NaN → false).
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Exact(e) => !e.is_zero(),
            Self::Real(RealNumber::Machine(n)) => *n != 0.0 && !n.is_nan(),
        }
    }

    /// Integer exponent when representable as `i64`.
    pub fn as_integer_exp(&self) -> Option<i64> {
        match self {
            Self::Exact(e) => e.as_integer_exp(),
            _ => None,
        }
    }

    /// Exact integer when representable as `i64`.
    pub fn as_exact_integer(&self) -> Option<i64> {
        self.as_integer_exp()
    }
}

fn format_machine(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") }
}

fn gcd_i64(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    if a == 0 { 1 } else { a }
}

fn is_decimal_int_wire(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    let body = match t.as_bytes()[0] {
        b'+' | b'-' => &t[1..],
        _ => t,
    };
    !body.is_empty() && body.chars().all(|c| c.is_ascii_digit())
}

fn normalize_int_wire(s: &str) -> String {
    let t = s.trim();
    let (neg, body) = match t.as_bytes()[0] {
        b'+' => (false, &t[1..]),
        b'-' => (true, &t[1..]),
        _ => (false, t),
    };
    let trimmed = body.trim_start_matches('0');
    let core = if trimmed.is_empty() { "0" } else { trimmed };
    if core == "0" {
        "0".to_string()
    }
    else if neg {
        format!("-{core}")
    }
    else {
        core.to_string()
    }
}

fn decimal_is_zero(s: &str) -> bool {
    decimal_as_i64(s) == Some(0) || normalize_int_wire(s) == "0"
}

fn decimal_as_i64(s: &str) -> Option<i64> {
    let t = s.trim();
    if !is_decimal_int_wire(t) {
        return None;
    }
    t.parse::<i64>().ok()
}
