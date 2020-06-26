//! 协作式取消令牌（VM 层；不依赖 `athena-numeric`）。

use std::{cell::Cell, rc::Rc};

use athena_types::{Diagnostic, DiagnosticCode, Result};

/// 可克隆共享的取消标志（单线程 Session 合同：`Rc` + `Cell`）。
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Rc<Cell<bool>>,
}

impl CancellationToken {
    /// 新建未取消令牌。
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求取消（幂等）。
    #[inline]
    pub fn cancel(&self) {
        self.cancelled.set(true);
    }

    /// 清除取消（测试 / 复用）。
    #[inline]
    pub fn reset(&self) {
        self.cancelled.set(false);
    }

    /// 是否已取消。
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.get()
    }

    /// 已取消则返回诊断。
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("component", "athena-vm").detail("reason", "cancelled"))
        }
        else {
            Ok(())
        }
    }
}
