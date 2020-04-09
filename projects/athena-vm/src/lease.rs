//! 执行期 root lease（RAII · 不拥有持久 payload）。

use std::{cell::RefCell, ptr::NonNull, rc::Rc};

use athena_gc::{GcHeap, GcObjectId, RootKind, RootToken};

/// 单次 `VmExecutor::execute` / reference 解释期间的 root 登记。
///
/// Drop 时注销全部本 lease 登记的 object / numeric root。禁止把 lease 做成第二套 GC。
pub struct ExecutionLease {
    heap: Rc<RefCell<GcHeap>>,
    object_roots: Vec<RootToken>,
    numeric_roots: Vec<RootToken>,
}

impl core::fmt::Debug for ExecutionLease {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExecutionLease")
            .field("object_roots", &self.object_roots.len())
            .field("numeric_roots", &self.numeric_roots.len())
            .finish_non_exhaustive()
    }
}

impl ExecutionLease {
    /// 绑定 Session / 宿主 heap。
    #[inline]
    pub fn new(heap: Rc<RefCell<GcHeap>>) -> Self {
        Self {
            heap,
            object_roots: Vec::new(),
            numeric_roots: Vec::new(),
        }
    }

    /// 登记 object root（默认 [`RootKind::InFlight`]）。
    pub fn register_object(&mut self, object: GcObjectId) -> RootToken {
        self.register_object_kind(object, RootKind::InFlight)
    }

    /// 登记 object root（显式种类）。
    pub fn register_object_kind(&mut self, object: GcObjectId, kind: RootKind) -> RootToken {
        let token = self.heap.borrow_mut().roots_mut().register(object, kind);
        self.object_roots.push(token);
        token
    }

    /// 登记 numeric payload root（默认 [`RootKind::InFlight`]）。
    pub fn register_numeric(&mut self, payload: NonNull<u8>) -> RootToken {
        self.register_numeric_kind(payload, RootKind::InFlight)
    }

    /// 登记 numeric payload root（显式种类）。
    pub fn register_numeric_kind(&mut self, payload: NonNull<u8>, kind: RootKind) -> RootToken {
        let token = self.heap.borrow_mut().roots_mut().register_numeric(payload, kind);
        self.numeric_roots.push(token);
        token
    }

    /// 已登记 object root 数量。
    #[inline]
    pub fn object_root_count(&self) -> usize {
        self.object_roots.len()
    }

    /// 已登记 numeric root 数量。
    #[inline]
    pub fn numeric_root_count(&self) -> usize {
        self.numeric_roots.len()
    }

    /// 提前释放全部 root（Drop 也会调用）。
    pub fn release_all(&mut self) {
        let mut heap = self.heap.borrow_mut();
        let roots = heap.roots_mut();
        for token in self.object_roots.drain(..) {
            let _ = roots.unregister(token);
        }
        for token in self.numeric_roots.drain(..) {
            let _ = roots.unregister_numeric(token);
        }
    }
}

impl Drop for ExecutionLease {
    fn drop(&mut self) {
        self.release_all();
    }
}
