//! Thread-local scratch borrow for convenience wrappers.

use std::cell::RefCell;

use crate::{kernel::ScratchWorkspace, policy::execution_budget::ExecutionBudget};

thread_local! {
    static KERNEL_SCRATCH: RefCell<ScratchWorkspace> = RefCell::new(ScratchWorkspace::default());
}

/// 借用线程本地 scratch 执行 kernel（调用结束清空）。
pub(crate) fn with_kernel_scratch<R>(
    budget: &ExecutionBudget,
    f: impl FnOnce(&mut ScratchWorkspace, &ExecutionBudget) -> R,
) -> R {
    KERNEL_SCRATCH.with(|cell| {
        if let Ok(mut scratch) = cell.try_borrow_mut() {
            let result = f(&mut *scratch, budget);
            scratch.clear();
            result
        }
        else {
            let mut scratch = ScratchWorkspace::default();
            f(&mut scratch, budget)
        }
    })
}
