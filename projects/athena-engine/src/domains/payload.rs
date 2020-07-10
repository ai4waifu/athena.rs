//! Session 局部的领域请求载荷仓（供 `CallProvider` 描述符绑定）。
//!
//! **禁止**经 host `pending_domain` 侧通道注入。`ProviderCallDescriptor::payload`
//! 必须指向本仓中的 [`DomainPayloadId`]。

use super::dispatch::DomainRequest;

/// Session 局部领域请求载荷句柄（写入 `ProviderCallDescriptor`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DomainPayloadId(pub u64);

/// [`DomainRequest`] 的 Session 仓（IR 描述符只持句柄）。
#[derive(Debug, Default)]
pub struct DomainPayloadStore {
    entries: Vec<DomainRequest>,
}

impl DomainPayloadStore {
    /// 空仓。
    pub fn new() -> Self {
        Self::default()
    }

    /// 条目数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Intern 请求并返回句柄（每次分配新槽，不按内容去重）。
    ///
    /// Goal module 与请求一一对应；去重会错误合并不同执行实例。
    pub fn intern(&mut self, request: DomainRequest) -> DomainPayloadId {
        let id = self.entries.len() as u64;
        self.entries.push(request);
        DomainPayloadId(id)
    }

    /// 按句柄借用请求。
    pub fn get(&self, id: DomainPayloadId) -> Option<&DomainRequest> {
        self.entries.get(id.0 as usize)
    }
}
