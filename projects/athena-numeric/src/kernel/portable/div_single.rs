//! Single-limb multi-precision division.

use athena_types::Result;

use crate::{kernel::LimbBuffer, policy::execution_budget::ExecutionBudget};

use super::primitive::effective_len;

pub(super) fn div_rem_1_into(u: &[u64], d: u64, q_out: &mut LimbBuffer, r_out: &mut LimbBuffer, budget: &ExecutionBudget) -> Result<()> {
    assert_ne!(d, 0);
    let la = effective_len(u);
    if la == 1 && u[0] < d {
        q_out.set_zero(budget)?;
        r_out.copy_canonical(u, budget)?;
        return Ok(());
    }
    let q_storage = q_out.storage_mut(la, budget)?;
    q_storage.fill(0);
    let mut rem: u128 = 0;
    for i in (0..la).rev() {
        rem = (rem << 64) | u128::from(u[i]);
        let qi = rem / u128::from(d);
        rem %= u128::from(d);
        q_storage[i] = qi as u64;
    }
    q_out.trim_canonical();
    r_out.copy_canonical(&[rem as u64], budget)?;
    Ok(())
}
