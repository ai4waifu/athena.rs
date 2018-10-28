//! 规范多项式稳定 hash（M-Graph / 缓存键）。

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::{expr::Polynomial, ring::CoefficientDomain, ring_table::RingTable};

/// 对 canonical 多项式求稳定结构 hash（含环 id 与系数域标签）。
pub fn canonical_hash(poly: &Polynomial, rings: &RingTable) -> Result<u64> {
    let desc = rings.get(poly.ring).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "canonical_hash_unknown_ring")
    })?;
    let mut h = DefaultHasher::new();
    poly.ring.0.hash(&mut h);
    hash_coefficient_domain(&desc.coefficients, &mut h);
    desc.variables.iter().for_each(|s| s.0.hash(&mut h));
    hash_order_tag(&desc.order, &mut h);
    "terms".hash(&mut h);
    for term in &poly.terms {
        term.coefficient.to_render_string().hash(&mut h);
        for e in &term.exponents {
            e.hash(&mut h);
        }
    }
    Ok(h.finish())
}

fn hash_coefficient_domain(domain: &CoefficientDomain, h: &mut DefaultHasher) {
    match domain {
        CoefficientDomain::Integer => "Z".hash(h),
        CoefficientDomain::Rational => "Q".hash(h),
        CoefficientDomain::PrimeField { p } => {
            "Fp".hash(h);
            p.to_decimal_string().hash(h);
        }
        CoefficientDomain::ModularInteger { modulus } => {
            "Zn".hash(h);
            modulus.value().to_decimal_string().hash(h);
        }
        CoefficientDomain::FiniteField { field, characteristic } => {
            "Fq".hash(h);
            field.0.hash(h);
            characteristic.to_decimal_string().hash(h);
        }
        CoefficientDomain::ApproximateReal => "R_approx".hash(h),
    }
}

fn hash_order_tag(order: &super::order::MonomialOrder, h: &mut DefaultHasher) {
    use super::order::MonomialOrder;
    match order {
        MonomialOrder::Lex => "lex".hash(h),
        MonomialOrder::GrLex => "grlex".hash(h),
        MonomialOrder::GrevLex => "grevlex".hash(h),
        MonomialOrder::Weighted { weights } => {
            "w".hash(h);
            for w in weights {
                w.hash(h);
            }
        }
        MonomialOrder::Block { blocks } => {
            "block".hash(h);
            blocks.len().hash(h);
            for b in blocks {
                hash_order_tag(b, h);
            }
        }
        MonomialOrder::Elimination { eliminate, rest } => {
            "elim".hash(h);
            eliminate.hash(h);
            hash_order_tag(rest, h);
        }
    }
}
