//! 泛化 tracing 合同（GC 不懂 Integer / 图论语义）。

use crate::ids::GcObjectId;

/// 标记工作台。
pub trait Tracer {
    /// 标记对象可达。
    fn mark_object(&mut self, id: GcObjectId);

    /// 标记 numeric / object allocation（payload 起点，header 在前方）。
    fn mark_allocation(&mut self, payload: *const u8);
}

/// 对象实现：声明出边。
pub trait Trace {
    /// 向 tracer 报告子引用与 numeric block。
    fn trace(&self, tracer: &mut dyn Tracer);
}

/// 上层图：按 `GcObjectId` 展开出边（collect 时注入）。
pub trait ObjectGraph {
    /// 追溯单个对象的引用边。
    fn trace_object(&self, id: GcObjectId, tracer: &mut dyn Tracer);
}

/// 空图（仅 root registry + 显式 mark_allocation）。
pub struct EmptyObjectGraph;

impl ObjectGraph for EmptyObjectGraph {
    fn trace_object(&self, _id: GcObjectId, _tracer: &mut dyn Tracer) {}
}
