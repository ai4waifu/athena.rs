//! Context 级绑定的 machine kernel 表（热路径只调已绑定条目）。

use athena_types::Result;

use crate::{
    dispatch::MachineCapability,
    kernel::{
        buffer::{LimbBuffer, ScratchWorkspace},
        portable::{self, LimbKernel, PortableLimbKernel},
        token::ExecutionToken,
    },
    policy::execution_budget::ExecutionBudget,
};

type BinOp = fn(&[u64], &[u64], &mut LimbBuffer, &mut ScratchWorkspace, &ExecutionBudget) -> Result<()>;
type Mul1Op = fn(&[u64], u64, &mut LimbBuffer, &mut ScratchWorkspace, &ExecutionBudget) -> Result<()>;
type SqrOp = fn(&[u64], &mut LimbBuffer, &mut ScratchWorkspace, &ExecutionBudget) -> Result<()>;
type DivOp = fn(&[u64], &[u64], &mut LimbBuffer, &mut LimbBuffer, &mut ScratchWorkspace, &ExecutionBudget) -> Result<()>;
type Add1Op = fn(u64, u64) -> (u64, u64);
type Mul1x1Op = fn(u64, u64) -> u128;

/// 已绑定的 limb 内核表（无所有权、无分配、无 GC）。
///
/// 公开入口一律要求 [`ExecutionToken`]：证明 pin / 容量 / 本次禁止 GC。
#[derive(Clone, Copy)]
pub struct KernelTable {
    id: &'static str,
    add_into: BinOp,
    sub_into: BinOp,
    mul_into: BinOp,
    mul_1_into: Mul1Op,
    sqr_into: SqrOp,
    div_rem_into: DivOp,
    add_1: Add1Op,
    mul_1x1: Mul1x1Op,
}

impl core::fmt::Debug for KernelTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KernelTable").field("id", &self.id).finish()
    }
}

impl PartialEq for KernelTable {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for KernelTable {}

impl KernelTable {
    /// 纯 Rust 语义基线表。
    pub fn pure_rust() -> Self {
        Self {
            id: "pure_rust",
            add_into: <PortableLimbKernel as LimbKernel>::add_into,
            sub_into: <PortableLimbKernel as LimbKernel>::sub_into,
            mul_into: <PortableLimbKernel as LimbKernel>::mul_into,
            mul_1_into: <PortableLimbKernel as LimbKernel>::mul_1_into,
            sqr_into: <PortableLimbKernel as LimbKernel>::sqr_into,
            div_rem_into: <PortableLimbKernel as LimbKernel>::div_rem_into,
            add_1: portable::add_1,
            mul_1x1: portable::mul_1x1,
        }
    }

    /// ISA / 测试组装入口。
    pub(crate) fn from_parts(
        id: &'static str,
        add_into: BinOp,
        sub_into: BinOp,
        mul_into: BinOp,
        mul_1_into: Mul1Op,
        sqr_into: SqrOp,
        div_rem_into: DivOp,
        add_1: Add1Op,
        mul_1x1: Mul1x1Op,
    ) -> Self {
        Self { id, add_into, sub_into, mul_into, mul_1_into, sqr_into, div_rem_into, add_1, mul_1x1 }
    }

    /// 按已冻结的 [`MachineCapability`] 绑定（context 创建时调用一次）。
    pub fn bind(machine: MachineCapability) -> Self {
        #[cfg(all(target_arch = "x86_64", not(target_family = "wasm")))]
        {
            if machine.adx || machine.bmi2 {
                return crate::kernel::x86_64::kernel_table();
            }
        }
        let _ = machine;
        Self::pure_rust()
    }

    /// 稳定 id。
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// `add_into`。
    pub fn add_into(
        &self,
        _token: ExecutionToken<'_>,
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        (self.add_into)(a, b, out, scratch, budget)
    }

    /// `sub_into`。
    pub fn sub_into(
        &self,
        _token: ExecutionToken<'_>,
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        (self.sub_into)(a, b, out, scratch, budget)
    }

    /// `mul_into`。
    pub fn mul_into(
        &self,
        _token: ExecutionToken<'_>,
        a: &[u64],
        b: &[u64],
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        (self.mul_into)(a, b, out, scratch, budget)
    }

    /// `mul_1_into`。
    pub fn mul_1_into(
        &self,
        _token: ExecutionToken<'_>,
        a: &[u64],
        limb: u64,
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        (self.mul_1_into)(a, limb, out, scratch, budget)
    }

    /// `sqr_into`。
    pub fn sqr_into(
        &self,
        _token: ExecutionToken<'_>,
        a: &[u64],
        out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        (self.sqr_into)(a, out, scratch, budget)
    }

    /// `div_rem_into`。
    pub fn div_rem_into(
        &self,
        _token: ExecutionToken<'_>,
        u: &[u64],
        v: &[u64],
        q_out: &mut LimbBuffer,
        r_out: &mut LimbBuffer,
        scratch: &mut ScratchWorkspace,
        budget: &ExecutionBudget,
    ) -> Result<()> {
        (self.div_rem_into)(u, v, q_out, r_out, scratch, budget)
    }

    /// 单 limb 加法。
    pub fn add_1(&self, _token: ExecutionToken<'_>, a: u64, b: u64) -> (u64, u64) {
        (self.add_1)(a, b)
    }

    /// 单 limb 乘法 → u128。
    pub fn mul_1x1(&self, _token: ExecutionToken<'_>, a: u64, b: u64) -> u128 {
        (self.mul_1x1)(a, b)
    }
}

impl Default for KernelTable {
    fn default() -> Self {
        Self::pure_rust()
    }
}
