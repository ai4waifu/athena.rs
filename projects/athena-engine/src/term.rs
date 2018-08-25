//! Runtime expression tree [`Term`] for transitional eval (not arena IR).

use std::fmt;

use num_bigint::BigInt;
use num_rational::BigRational;

use athena_types::Number;

/// Extract kernel number from a term atom.
pub fn number_from_term(term: &Term) -> Option<&Number> {
    match term {
        Term::Atom(Atom::Number(n)) => Some(n),
        _ => None,
    }
}

/// Atomic value in the engine IR.
#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    /// Unified kernel number (sole numeric truth source).
    Number(Number),
    /// String value (already decoded by the host/dialect layer).
    String(String),
    /// Symbol name.
    Symbol(String),
}

/// Runtime expression tree for transitional eval (not dialect AST, not arena IR).
#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    /// Atom.
    Atom(Atom),
    /// Ordered collection.
    List(Vec<Term>),
    /// Application `head(args…)`.
    Application {
        /// Head term (usually a symbol).
        head: Box<Term>,
        /// Arguments.
        arguments: Vec<Term>,
    },
}

impl Term {
    /// Symbol atom.
    pub fn symbol(name: impl Into<String>) -> Self {
        Self::Atom(Atom::Symbol(name.into()))
    }

    /// Small exact integer convenience.
    pub fn int(n: i64) -> Self {
        Self::number(Number::small_int(n))
    }

    /// Arbitrary-precision exact integer.
    pub fn integer(n: impl Into<BigInt>) -> Self {
        Self::number(Number::integer(n))
    }

    /// Exact rational (normalized).
    pub fn rational(r: BigRational) -> Self {
        Self::number(Number::rational(r))
    }

    /// Machine real from an already-decoded `f64` value.
    pub fn real(n: f64) -> Self {
        Self::number(Number::machine(n))
    }

    /// Unified number atom from an already-decoded [`Number`].
    pub fn number(n: Number) -> Self {
        Self::Atom(Atom::Number(n))
    }

    /// `head(args…)` with symbol head.
    pub fn app(head: impl Into<String>, args: Vec<Term>) -> Self {
        Self::Application { head: Box::new(Self::symbol(head)), arguments: args }
    }

    /// Head symbol name, if any.
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

    /// Kernel number reference (no precision loss).
    pub fn as_number(&self) -> Option<&Number> {
        number_from_term(self)
    }

    /// Lossy `f64` — display / host hints only.
    pub fn as_f64_lossy(&self) -> Option<f64> {
        self.as_number().and_then(Number::to_f64_lossy)
    }

    /// Whether this is the given symbol.
    pub fn is_symbol(&self, name: &str) -> bool {
        matches!(self, Self::Atom(Atom::Symbol(s)) if s == name)
    }

    /// Whether numeric `-1`.
    pub fn is_neg_one(&self) -> bool {
        self.as_number().is_some_and(Number::is_neg_one)
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
