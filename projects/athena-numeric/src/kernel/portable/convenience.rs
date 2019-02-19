//! Non-hot-path `Vec` convenience wrappers (tests / legacy callers).

use athena_types::Result;

use crate::algorithm::AlgorithmPlanner;
use crate::dispatch::CapabilityBundle;
use crate::kernel::LimbBuffer;
use crate::policy::execution_budget::ExecutionBudget;

use super::glue::{LimbKernel, PortableLimbKernel};
use super::mul_schoolbook::{addmul_1_inplace, mul_schoolbook_into, sqr_schoolbook_into};
use super::primitive::{effective_len, is_zero, normalize_trim};
use super::scratch_tls::with_kernel_scratch;

fn default_planner() -> AlgorithmPlanner {
    AlgorithmPlanner::new(CapabilityBundle::portable_default())
}

/// 便利：分配新 `Vec` 的加法（**非热路径**；值层请用 `*_into` / executor）。
pub(crate) fn add_n_budgeted(a: &[u64], b: &[u64], budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::with_capacity(a.len().max(b.len()) + 1, budget)?;
        PortableLimbKernel::add_into(a, b, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn add_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    add_n_budgeted(a, b, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn sub_n_budgeted(a: &[u64], b: &[u64], budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::with_capacity(a.len(), budget)?;
        PortableLimbKernel::sub_into(a, b, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn sub_n(a: &[u64], b: &[u64]) -> Vec<u64> {
    sub_n_budgeted(a, b, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn mul_budgeted(a: &[u64], b: &[u64], budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::zero();
        let strategy = default_planner().plan_mul(effective_len(a), effective_len(b));
        PortableLimbKernel::mul_into(a, b, strategy, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    mul_budgeted(a, b, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn mul_1_budgeted(a: &[u64], n: u64, budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::zero();
        PortableLimbKernel::mul_1_into(a, n, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn mul_1(a: &[u64], n: u64) -> Vec<u64> {
    mul_1_budgeted(a, n, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn sqr_budgeted(a: &[u64], budget: &ExecutionBudget) -> Result<Vec<u64>> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut out = LimbBuffer::zero();
        let n = effective_len(a);
        let strategy = default_planner().plan_mul(n, n);
        PortableLimbKernel::sqr_into(a, strategy, &mut out, scratch, budget)?;
        Ok(out.into_canonical_vec())
    })
}

pub(crate) fn sqr(a: &[u64]) -> Vec<u64> {
    sqr_budgeted(a, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn div_rem_budgeted(u: &[u64], v: &[u64], budget: &ExecutionBudget) -> Result<(Vec<u64>, Vec<u64>)> {
    with_kernel_scratch(budget, |scratch, budget| {
        let mut q = LimbBuffer::zero();
        let mut r = LimbBuffer::zero();
        let strategy = default_planner().plan_div(effective_len(u), effective_len(v));
        PortableLimbKernel::div_rem_into(u, v, strategy, &mut q, &mut r, scratch, budget)?;
        Ok((q.into_canonical_vec(), r.into_canonical_vec()))
    })
}

pub(crate) fn div_rem(u: &[u64], v: &[u64]) -> (Vec<u64>, Vec<u64>) {
    div_rem_budgeted(u, v, &ExecutionBudget::unlimited()).expect("unlimited")
}

pub(crate) fn addmul_1(r: &[u64], a: &[u64], n: u64) -> Vec<u64> {
    assert_ne!(n, 0);
    if is_zero(a) {
        return normalize_trim(r.to_vec());
    }
    let la = effective_len(a);
    let lr = effective_len(r);
    let mut out = r.to_vec();
    out.resize(lr.max(la) + 1, 0);
    addmul_1_inplace(&mut out, a, n);
    normalize_trim(out)
}

pub(crate) fn mul_schoolbook(a: &[u64], b: &[u64]) -> Vec<u64> {
    let la = effective_len(a);
    let lb = effective_len(b);
    let mut out = vec![0u64; (la + lb).max(1)];
    mul_schoolbook_into(a, b, &mut out);
    normalize_trim(out)
}

pub(crate) fn sqr_schoolbook(a: &[u64]) -> Vec<u64> {
    let la = effective_len(a);
    let mut out = vec![0u64; (2 * la).max(1)];
    sqr_schoolbook_into(a, &mut out);
    normalize_trim(out)
}

pub(crate) fn karatsuba_mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    mul(a, b)
}
