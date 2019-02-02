//! 协作式取消令牌（Session / `NumericContext` 共享）。

use std::{cell::Cell, rc::Rc};

use athena_types::{Diagnostic, DiagnosticCode, Result};

/// 可克隆共享的取消标志（单线程 Session 合同：`Rc` + `Cell`）。
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Rc<Cell<bool>>,
}

impl CancellationToken {
    /// 新建未取消令牌。
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求取消（幂等）。
    pub fn cancel(&self) {
        self.cancelled.set(true);
    }

    /// 清除取消（测试 / 新请求复用 Session 时）。
    pub fn reset(&self) {
        self.cancelled.set(false);
    }

    /// 是否已取消。
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.get()
    }

    /// 已取消则返回诊断。
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Diagnostic::new(DiagnosticCode::NumericCancelled).detail("domain", "numeric").detail("kind", "cancelled"))
        }
        else {
            Ok(())
        }
    }
}
