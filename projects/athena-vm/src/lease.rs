//! 执行期 root lease（RAII · 不拥有持久 payload）。

use std::{cell::RefCell, ptr::NonNull, rc::Rc};

use athena_gc::{GcHeap, GcObjectId, RootKind, RootToken};
use athena_types::TermId;

/// 单次 `VmExecutor::execute` / reference 解释期间的 root 登记。
///
/// Drop 时注销全部本 lease 登记的 object / numeric root，并清空 Term pin。
/// 禁止把 lease 做成第二套 GC。
///
/// **过渡**：`TermStore` 尚未 GC-backed 时，[`Self::register_term`] 只做执行期 pin 记账，
/// 证明 captured / 常量 `TermId` 在解释期间被显式持有；TermStore 闭合后改为真 root。
pub struct ExecutionLease {
    heap: Rc<RefCell<GcHeap>>,
    object_roots: Vec<RootToken>,
    numeric_roots: Vec<RootToken>,
    term_pins: Vec<TermId>,
}

impl core::fmt::Debug for ExecutionLease {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ExecutionLease")
            .field("object_roots", &self.object_roots.len())
            .field("numeric_roots", &self.numeric_roots.len())
            .field("term_pins", &self.term_pins.len())
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
            term_pins: Vec::new(),
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

    /// 执行期 pin 一个 `TermId`（过渡：非 GC root，仅 lease 生命周期记账）。
    pub fn register_term(&mut self, term: TermId) {
        self.term_pins.push(term);
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

    /// 已 pin 的 Term 数量。
    #[inline]
    pub fn term_pin_count(&self) -> usize {
        self.term_pins.len()
    }

    /// 当前 Term pin 快照（测试 / 诊断）。
    #[inline]
    pub fn term_pins(&self) -> &[TermId] {
        &self.term_pins
    }

    /// 提前释放全部 root 与 Term pin（Drop 也会调用）。
    pub fn release_all(&mut self) {
        let mut heap = self.heap.borrow_mut();
        let roots = heap.roots_mut();
        for token in self.object_roots.drain(..) {
            let _ = roots.unregister(token);
        }
        for token in self.numeric_roots.drain(..) {
            let _ = roots.unregister_numeric(token);
        }
        self.term_pins.clear();
    }
}

impl Drop for ExecutionLease {
    fn drop(&mut self) {
        self.release_all();
    }
}
