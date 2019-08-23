//! Closed core semantic operators and application heads.
//!
//! Core math / logic / structure ops are [`SemanticOperator`].
//! [`OperatorRegistry`] is only for extension display names, never the core catalog.

use athena_types::OperatorId;

/// Closed Athena core semantic operator identity (fingerprint-stable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticOperator {
    // arithmetic
    /// `a + b + …`
    Add,
    /// `a - b` or unary minus form.
    Subtract,
    /// `a * b * …`
    Multiply,
    /// `a / b`
    Divide,
    /// `a ^ b`
    Power,
    /// unary negation.
    Negate,
    /// elementwise multiply.
    ElementwiseMultiply,
    /// elementwise divide.
    ElementwiseDivide,
    /// elementwise power.
    ElementwisePower,
    // compare / logic
    /// structural / numeric equality.
    Equal,
    /// inequality.
    Unequal,
    /// identical (same structure / slot identity).
    Identical,
    /// `<`
    Less,
    /// `>`
    Greater,
    /// `<=`
    LessEqual,
    /// `>=`
    GreaterEqual,
    /// boolean and.
    And,
    /// boolean or.
    Or,
    /// boolean not.
    Not,
    /// true-query.
    TrueQ,
    // structure
    /// absolute value.
    Abs,
    /// collection length.
    Length,
    /// first element.
    First,
    /// rest of collection.
    Rest,
    /// factorial.
    Factorial,
    /// square root.
    Sqrt,
    /// join collections.
    Join,
    /// integer range.
    Range,
    /// apply head to args.
    Apply,
    /// apply-head / application form wrapper.
    ApplyHead,
    /// size / dimensions.
    Size,
    /// summation.
    Sum,
    /// product.
    Product,
    /// matrix determinant.
    Determinant,
    /// map over collection.
    Map,
    /// zero matrix / array constructor.
    Zeros,
    /// ones matrix / array constructor.
    Ones,
    /// identity matrix constructor.
    Eye,
    /// immediate rewrite rule.
    Rule,
    /// deferred rewrite rule.
    RuleDeferred,
    /// replace-all.
    ReplaceAll,
    /// collect pattern matches.
    CollectMatches,
    /// match predicate.
    Matches,
    /// simplify.
    Simplify,
    /// hold / quote arguments.
    Hold,
    /// anonymous function binder.
    Function,
}

impl SemanticOperator {
    /// Stable discriminant for fingerprints (do not renumber lightly).
    pub const fn discriminant(self) -> u32 {
        match self {
            Self::Add => 1,
            Self::Subtract => 2,
            Self::Multiply => 3,
            Self::Divide => 4,
            Self::Power => 5,
            Self::Negate => 6,
            Self::ElementwiseMultiply => 7,
            Self::ElementwiseDivide => 8,
            Self::ElementwisePower => 9,
            Self::Equal => 10,
            Self::Unequal => 11,
            Self::Identical => 12,
            Self::Less => 13,
            Self::Greater => 14,
            Self::LessEqual => 15,
            Self::GreaterEqual => 16,
            Self::And => 17,
            Self::Or => 18,
            Self::Not => 19,
            Self::TrueQ => 20,
            Self::Abs => 21,
            Self::Length => 22,
            Self::First => 23,
            Self::Rest => 24,
            Self::Factorial => 25,
            Self::Sqrt => 26,
            Self::Join => 27,
            Self::Range => 28,
            Self::Apply => 29,
            Self::ApplyHead => 30,
            Self::Size => 31,
            Self::Sum => 32,
            Self::Product => 33,
            Self::Determinant => 34,
            Self::Map => 35,
            Self::Zeros => 36,
            Self::Ones => 37,
            Self::Eye => 38,
            Self::Rule => 39,
            Self::RuleDeferred => 40,
            Self::ReplaceAll => 41,
            Self::CollectMatches => 42,
            Self::Matches => 43,
            Self::Simplify => 44,
            Self::Hold => 45,
            Self::Function => 46,
        }
    }

    /// Neutral debug label (`Add`, not dialect `Plus`).
    pub const fn debug_label(self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Multiply => "Multiply",
            Self::Divide => "Divide",
            Self::Power => "Power",
            Self::Negate => "Negate",
            Self::ElementwiseMultiply => "ElementwiseMultiply",
            Self::ElementwiseDivide => "ElementwiseDivide",
            Self::ElementwisePower => "ElementwisePower",
            Self::Equal => "Equal",
            Self::Unequal => "Unequal",
            Self::Identical => "Identical",
            Self::Less => "Less",
            Self::Greater => "Greater",
            Self::LessEqual => "LessEqual",
            Self::GreaterEqual => "GreaterEqual",
            Self::And => "And",
            Self::Or => "Or",
            Self::Not => "Not",
            Self::TrueQ => "TrueQ",
            Self::Abs => "Abs",
            Self::Length => "Length",
            Self::First => "First",
            Self::Rest => "Rest",
            Self::Factorial => "Factorial",
            Self::Sqrt => "Sqrt",
            Self::Join => "Join",
            Self::Range => "Range",
            Self::Apply => "Apply",
            Self::ApplyHead => "ApplyHead",
            Self::Size => "Size",
            Self::Sum => "Sum",
            Self::Product => "Product",
            Self::Determinant => "Determinant",
            Self::Map => "Map",
            Self::Zeros => "Zeros",
            Self::Ones => "Ones",
            Self::Eye => "Eye",
            Self::Rule => "Rule",
            Self::RuleDeferred => "RuleDeferred",
            Self::ReplaceAll => "ReplaceAll",
            Self::CollectMatches => "CollectMatches",
            Self::Matches => "Matches",
            Self::Simplify => "Simplify",
            Self::Hold => "Hold",
            Self::Function => "Function",
        }
    }
}

/// Application head: closed semantic op or extension-only identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApplicationHead {
    /// Core closed semantic operator.
    Semantic(SemanticOperator),
    /// Extension-only identity. Display name may live in [`super::OperatorRegistry`].
    Extension(OperatorId),
}
