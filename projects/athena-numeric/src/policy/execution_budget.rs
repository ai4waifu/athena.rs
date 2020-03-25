//! 数值内核执行的分配与增长预算 + runtime heap / scratch / cancel。

use std::{cell::RefCell, rc::Rc};

use athena_gc::{GcHeap, GcMode, HeapBudget, ScratchArena, ScratchMark, TemporaryNumericBlock};
use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    dispatch::{CapabilityBundle, MachineCapability, NumericBackend, NumericBackendLimits, PortableBackend},
    kernel::{ExecutionToken, KernelTable, ScratchWorkspace, token::KernelPreconditions},
    policy::cancel::CancellationToken,
};

/// 由 backend 上限或 Session 策略接入的执行预算。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionBudget {
    max_limbs: Option<u32>,
    max_significand_bits: Option<u32>,
    max_wire_payload_bytes: Option<u32>,
}

impl ExecutionBudget {
    /// 无 limb / 载荷上限（开发 / 测试）。
    pub fn unlimited() -> Self {
        Self { max_limbs: None, max_significand_bits: None, max_wire_payload_bytes: None }
    }

    /// 由静态 backend 合同构造。
    pub fn from_limits(limits: &NumericBackendLimits) -> Self {
        Self {
            max_limbs: limits.max_limbs,
            max_significand_bits: limits.max_significand_bits,
            max_wire_payload_bytes: limits.max_wire_payload_bytes,
        }
    }

    /// 规范 limb 数上限（若有界）。
    pub fn max_limbs(&self) -> Option<u32> {
        self.max_limbs
    }

    /// 任意精度浮点最大尾数位宽。
    pub fn max_significand_bits(&self) -> Option<u32> {
        self.max_significand_bits
    }

    /// 解码用 wire 载荷最大字节数。
    pub fn max_wire_payload_bytes(&self) -> Option<u32> {
        self.max_wire_payload_bytes
    }

    /// 拒绝将容纳 `limbs` 个规范 limb 的缓冲。
    pub fn check_limbs(&self, limbs: usize) -> Result<()> {
        if let Some(max) = self.max_limbs {
            if limbs > max as usize {
                return Err(resource_limit("limbs", limbs, max));
            }
        }
        Ok(())
    }

    /// 拒绝宽于策略的尾数。
    pub fn check_significand_bits(&self, bits: u64) -> Result<()> {
        if let Some(max) = self.max_significand_bits {
            if bits > u64::from(max) {
                return Err(resource_limit("significand_bits", bits as usize, max));
            }
        }
        Ok(())
    }

    /// 拒绝大于策略的 wire 载荷。
    pub fn check_wire_bytes(&self, bytes: usize) -> Result<()> {
        if let Some(max) = self.max_wire_payload_bytes {
            if bytes > max as usize {
                return Err(resource_limit("wire_bytes", bytes, max));
            }
        }
        Ok(())
    }

    /// 估算并检查加法输出 limb 数。
    pub fn check_add(&self, a_limbs: usize, b_limbs: usize) -> Result<()> {
        let out = a_limbs.max(b_limbs) + 1;
        self.check_limbs(out)
    }

    /// 估算并检查乘法输出 limb 数。
    pub fn check_mul(&self, a_limbs: usize, b_limbs: usize) -> Result<()> {
        let out = a_limbs + b_limbs;
        self.check_limbs(out)
    }

    /// 估算并检查 Karatsuba 乘法 scratch（与 `karatsuba_scratch_limbs` 同构）。
    pub fn check_mul_scratch(&self, a_limbs: usize, b_limbs: usize) -> Result<()> {
        let n = a_limbs.max(b_limbs);
        if n < 32 {
            return self.check_limbs(a_limbs + b_limbs);
        }
        let mut width = n;
        let mut total = 0usize;
        while width >= 32 {
            let half = (width + 1) / 2;
            total = total.saturating_add(2 * half + 2 * half + (half + 1) + (half + 1) + (2 * half + 2));
            width = half + 1;
        }
        self.check_limbs(total.max(a_limbs + b_limbs))
    }

    /// 估算并检查除法商缓冲。
    pub fn check_div(&self, u_limbs: usize, v_limbs: usize) -> Result<()> {
        let q = if v_limbs == 0 { u_limbs + 1 } else { u_limbs.saturating_sub(v_limbs) + 1 };
        self.check_limbs(q.max(u_limbs) + v_limbs + 2)
    }
}

/// 数值执行上下文：预算 + 取消 + heap 分配 + kernel scratch + 冻结能力 / `KernelTable`。
///
/// `NumericContext ≠ allocator`：GC 策略在 `athena-gc` / engine；本类型只提供准入与钩子。
#[derive(Clone)]
pub struct NumericContext {
    budget: ExecutionBudget,
    heap: Rc<RefCell<GcHeap>>,
    cancel: CancellationToken,
    /// 值层 / kernel 共用的 limb scratch（非 GC tracing）。
    scratch: Rc<RefCell<ScratchWorkspace>>,
    /// 热路径复用的输出 `LimbBuffer`（避免每次 `publish` 系统堆 `Vec`）。
    out_buf: Rc<RefCell<crate::kernel::LimbBuffer>>,
    /// 第二输出缓冲（`div_rem` 余数等）。
    out_buf2: Rc<RefCell<crate::kernel::LimbBuffer>>,
    /// Context 创建时冻结的能力束。
    capabilities: CapabilityBundle,
    /// Context 创建时绑定的 machine kernel 表。
    kernels: KernelTable,
}

impl core::fmt::Debug for NumericContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NumericContext")
            .field("budget", &self.budget)
            .field("heap_id", &self.heap.borrow().id())
            .field("cancelled", &self.cancel.is_cancelled())
            .field("kernel", &self.kernels.id())
            .finish()
    }
}

impl NumericContext {
    fn assemble(budget: ExecutionBudget, heap: Rc<RefCell<GcHeap>>, capabilities: CapabilityBundle) -> Self {
        let kernels = KernelTable::bind(capabilities.machine);
        Self {
            budget,
            heap,
            cancel: CancellationToken::new(),
            scratch: Rc::new(RefCell::new(ScratchWorkspace::default())),
            out_buf: Rc::new(RefCell::new(crate::kernel::LimbBuffer::zero())),
            out_buf2: Rc::new(RefCell::new(crate::kernel::LimbBuffer::zero())),
            capabilities,
            kernels,
        }
    }

    /// 宿主可见便利入口：[`GcHeap::shared_default`] + Auto + portable 上限。
    ///
    /// 共享默认 heap 路径。Session 算术请用 [`Self::session_default`]。
    pub fn portable_default() -> Self {
        let mut caps = CapabilityBundle::portable_default();
        caps.resource = crate::dispatch::ResourceCapability::from_limits(NumericBackend::contract(&PortableBackend::default()).limits);
        Self::assemble(
            ExecutionBudget::from_limits(&NumericBackend::contract(&PortableBackend::default()).limits),
            GcHeap::shared_default(),
            caps,
        )
    }

    /// 由显式 backend 上限构造（线程默认 heap · Auto · portable kernel）。
    ///
    /// 与 [`Self::portable_default`] 同属共享默认 heap 语义。
    pub fn from_limits(limits: &NumericBackendLimits) -> Self {
        let mut caps = CapabilityBundle::portable_default();
        caps.resource = crate::dispatch::ResourceCapability::from_limits(*limits);
        Self::assemble(ExecutionBudget::from_limits(limits), GcHeap::shared_default(), caps)
    }

    /// 无限制预算 + 线程默认 heap（Auto）。仅测试与无 Session 的便利入口。
    ///
    /// Session / numeric 层请用 [`Self::session_default`]。
    pub fn unlimited() -> Self {
        let mut caps = CapabilityBundle::portable_default();
        caps.resource = crate::dispatch::ResourceCapability::unlimited();
        Self::assemble(ExecutionBudget::unlimited(), GcHeap::shared_default(), caps)
    }

    /// Session / numeric 层默认：隔离 heap + [`GcMode::Deferred`] + 无 limb 上限。
    ///
    /// 复用 context 算术发布目标。不等于 [`Self::portable_default`]（shared Auto）。
    pub fn session_default() -> Self {
        Self::session_with_heap_budget(HeapBudget::default())
    }

    /// 同 [`Self::session_default`]，可指定 heap 预算（Criterion 用 [`HeapBudget::for_microbench`]）。
    pub fn session_with_heap_budget(heap_budget: HeapBudget) -> Self {
        let heap = GcHeap::new_shared(heap_budget);
        heap.borrow().gc().set_base_mode(GcMode::Deferred);
        let mut caps = CapabilityBundle::portable_default();
        caps.resource = crate::dispatch::ResourceCapability::unlimited();
        Self::assemble(ExecutionBudget::unlimited(), heap, caps)
    }

    /// Kernel 微基准：隔离 heap + [`GcMode::Disabled`]（[`HeapBudget::for_microbench`]）。
    pub fn kernel_bench_context() -> Self {
        Self::kernel_bench_with_heap_budget(HeapBudget::for_microbench())
    }

    /// 同 [`Self::kernel_bench_context`]，可指定 heap 预算。
    pub fn kernel_bench_with_heap_budget(heap_budget: HeapBudget) -> Self {
        let heap = GcHeap::new_shared(heap_budget);
        heap.borrow().gc().set_base_mode(GcMode::Disabled);
        let mut caps = CapabilityBundle::portable_default();
        caps.resource = crate::dispatch::ResourceCapability::unlimited();
        Self::assemble(ExecutionBudget::unlimited(), heap, caps)
    }

    /// 显式绑定已有 heap（portable kernel）。调用方负责 `GcMode`。
    pub fn with_heap(budget: ExecutionBudget, heap: Rc<RefCell<GcHeap>>) -> Self {
        Self::assemble(budget, heap, CapabilityBundle::portable_default())
    }

    /// 新建隔离 heap（基准 mode 仍为 Auto，除非调用方再改）。
    pub fn with_new_heap(budget: ExecutionBudget, heap_budget: HeapBudget) -> Self {
        Self::assemble(budget, GcHeap::new_shared(heap_budget), CapabilityBundle::portable_default())
    }

    /// 显式能力束 + heap（绑定对应 `KernelTable`）。
    pub fn with_capabilities(budget: ExecutionBudget, heap: Rc<RefCell<GcHeap>>, capabilities: CapabilityBundle) -> Self {
        let mut caps = capabilities;
        // 资源上限与 ExecutionBudget 对齐。
        caps.resource.limits.max_limbs = budget.max_limbs();
        caps.resource.limits.max_significand_bits = budget.max_significand_bits();
        caps.resource.limits.max_wire_payload_bytes = budget.max_wire_payload_bytes();
        Self::assemble(budget, heap, caps)
    }

    /// 强制使用 portable `KernelTable`（parity / 差分）。
    pub fn with_portable_kernels(mut self) -> Self {
        self.capabilities.machine = MachineCapability::PORTABLE;
        self.kernels = KernelTable::portable();
        self
    }

    /// 冻结的能力束。
    pub fn capabilities(&self) -> CapabilityBundle {
        self.capabilities
    }

    /// Context 级算法规划器（唯一策略源）。
    pub fn planner(&self) -> crate::algorithm::AlgorithmPlanner {
        crate::algorithm::AlgorithmPlanner::new(self.capabilities)
    }

    /// 已绑定的 machine kernel 表。
    pub fn kernels(&self) -> KernelTable {
        self.kernels
    }

    /// 发一次 machine-kernel [`ExecutionToken`]（证明本次调用不触发 GC / 扩容）。
    ///
    /// Executor / value 层在已做预算检查后调用；kernel 不得持有完整 context。
    #[inline]
    pub fn kernel_token(&self) -> ExecutionToken<'_> {
        let _ = self;
        ExecutionToken::issue(KernelPreconditions { out_capacity: 1, out_need: 1 })
    }

    /// 绑定外部取消令牌（Session 级共享）。
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancel = token;
        self
    }

    /// 当前预算。
    pub fn budget(&self) -> &ExecutionBudget {
        &self.budget
    }

    /// Runtime heap。
    pub fn heap(&self) -> &Rc<RefCell<GcHeap>> {
        &self.heap
    }

    /// 取消令牌。
    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancel
    }

    /// 请求取消。
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// 是否已取消。
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// 已取消则失败。
    pub fn check_cancelled(&self) -> Result<()> {
        self.cancel.check()
    }

    /// 运算入口准入：取消 +（可选扩展）其它闸门。
    pub fn check_entry(&self) -> Result<()> {
        self.check_cancelled()
    }

    /// 借用 kernel limb scratch（统一钩子；调用方负责 `rewind`/`clear`）。
    pub fn with_scratch<R>(&self, f: impl FnOnce(&mut ScratchWorkspace) -> R) -> R {
        let mut scratch = self.scratch.borrow_mut();
        f(&mut scratch)
    }

    /// 在预算下执行并在结束后重置 scratch 游标。
    pub fn with_scratch_frame<R>(&self, f: impl FnOnce(&mut ScratchWorkspace, &ExecutionBudget) -> R) -> R {
        let mut scratch = self.scratch.borrow_mut();
        let result = f(&mut scratch, &self.budget);
        scratch.clear();
        result
    }

    /// 是否允许复用 context 级输出 `LimbBuffer`（destination reuse）。
    #[inline]
    pub fn can_reuse_destination(&self) -> bool {
        self.capabilities.resource.can_reuse_destination
    }

    /// 借用可复用输出缓冲（热路径 publish）。
    pub(crate) fn with_out_buf<R>(&self, f: impl FnOnce(&mut crate::kernel::LimbBuffer) -> R) -> R {
        let mut out = self.out_buf.borrow_mut();
        f(&mut out)
    }

    /// 借用第二输出缓冲（与 [`Self::with_out_buf`] 可嵌套）。
    pub(crate) fn with_out_buf2<R>(&self, f: impl FnOnce(&mut crate::kernel::LimbBuffer) -> R) -> R {
        let mut out = self.out_buf2.borrow_mut();
        f(&mut out)
    }

    /// GC heap 上的 byte scratch 水位。
    pub fn gc_scratch_mark(&self) -> ScratchMark {
        self.heap.borrow_mut().scratch().mark()
    }

    /// 回滚 GC scratch 到标记。
    pub fn gc_scratch_rewind(&self, mark: ScratchMark) {
        self.heap.borrow_mut().scratch().rewind(mark);
    }

    /// 在 GC scratch 上执行（自动 rewind）。
    pub fn with_gc_scratch<R>(&self, f: impl FnOnce(&mut ScratchArena) -> R) -> R {
        let mut heap = self.heap.borrow_mut();
        let mark = heap.scratch().mark();
        let result = f(heap.scratch());
        heap.scratch().rewind(mark);
        result
    }

    /// 经 context 分配临时 numeric limb block（检查取消）。
    pub fn allocate_numeric_block(&self, capacity_limbs: usize) -> Result<TemporaryNumericBlock> {
        self.check_entry()?;
        self.budget.check_limbs(capacity_limbs)?;
        self.heap.borrow_mut().allocate_numeric_block(capacity_limbs).map_err(|e| {
            Diagnostic::new(DiagnosticCode::NumericResourceLimit)
                .detail("domain", "numeric")
                .detail("kind", "gc_alloc")
                .detail("reason", e.to_string())
        })
    }
}

fn resource_limit(kind: &str, got: usize, max: u32) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericResourceLimit)
        .detail("domain", "numeric")
        .detail("kind", kind)
        .detail("got", got.to_string())
        .detail("max", max.to_string())
}
