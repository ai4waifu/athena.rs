//! 泛化 tracing 合同（GC 不懂 Integer / 图论语义）。

use crate::ids::GcObjectId;

/// 标记工作台。
pub trait Tracer {
    /// 标记对象可达。
    fn mark_object(&mut self, id: GcObjectId);

    /// 标记 numeric / object allocation（由 limb 或 payload 指针定位 header）。
    fn mark_allocation(&mut self, payload: *const u8);
}

/// 对象实现：声明出边。
pub trait Trace {
    /// 向 tracer 报告子引用与 numeric block。
    fn trace(&self, tracer: &mut dyn Tracer);
}
