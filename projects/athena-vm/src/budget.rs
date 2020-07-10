//! 可克隆共享的解释步数预算（VM 层；嵌套入口共用同一计数）。

use std::{cell::Cell, rc::Rc};

/// 跨嵌套 VM 入口共享的剩余步数。
///
/// `None` 表示不设上限。有上限时每次解释步进调用 [`Self::consume_one`]。
#[derive(Debug, Clone, Default)]
pub struct StepBudget {
    remaining: Rc<Cell<Option<u64>>>,
}

impl StepBudget {
    /// 无步数上限。
    #[inline]
    pub fn unlimited() -> Self {
        Self { remaining: Rc::new(Cell::new(None)) }
    }

    /// 有限剩余步数（嵌套入口共享同一计数）。
    #[inline]
    pub fn limited(max_steps: u64) -> Self {
        Self { remaining: Rc::new(Cell::new(Some(max_steps))) }
    }

    /// 当前剩余（`None` = 无上限）。
    #[inline]
    pub fn remaining(&self) -> Option<u64> {
        self.remaining.get()
    }

    /// 消耗一步。已耗尽则返回 `false`（调用方应退出 `BudgetExceeded`）。
    #[inline]
    pub fn consume_one(&self) -> bool {
        match self.remaining.get() {
            None => true,
            Some(0) => false,
            Some(n) => {
                self.remaining.set(Some(n - 1));
                true
            }
        }
    }
}
