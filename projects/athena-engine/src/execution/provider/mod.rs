//! `ExecutionIR` 的类型化 provider 调用面（能力 + 校验器交接）。
//!
//! Provider 私有的 kernel 产物不是第二套 Athena IR。它们仅绑定到
//! [`ProviderCallDescriptor`](crate::execution::ir::ProviderCallDescriptor) 描述符。

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use athena_types::ExtensionOperatorId;

use crate::execution::ir::{ExecutionValueType, ProviderCallDescriptor, ProviderCallId};

/// Provider 调用点所需的能力快照。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderCapabilitySnapshot {
    /// 不透明能力指纹（由后端定义各位）。
    pub fingerprint: u64,
}

impl ProviderCapabilitySnapshot {
    /// 由封闭算子 id 推导会话局部能力指纹。
    pub fn from_operator(operator: ExtensionOperatorId) -> Self {
        let mut hasher = DefaultHasher::new();
        0x5052_4f56_4341_5045u64.hash(&mut hasher); // "PROVCAPE"
        operator.0.hash(&mut hasher);
        Self { fingerprint: hasher.finish() }
    }
}

/// 从执行器到 provider 校验器 / 准入的交接。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCallHandoff {
    /// 来自 module 表的描述符。
    pub descriptor: ProviderCallDescriptor,
    /// 所需能力。
    pub capabilities: ProviderCapabilitySnapshot,
}

impl ProviderCallHandoff {
    /// 由算子标识构建交接。
    pub fn from_operator(id: ProviderCallId, operator: ExtensionOperatorId) -> Self {
        Self {
            descriptor: ProviderCallDescriptor::new(id, operator, ExecutionValueType::Unknown),
            capabilities: ProviderCapabilitySnapshot::from_operator(operator),
        }
    }

    /// 由已编译描述符表条目构建交接。
    pub fn from_descriptor(descriptor: ProviderCallDescriptor) -> Self {
        let capabilities = ProviderCapabilitySnapshot::from_operator(descriptor.operator);
        Self { descriptor, capabilities }
    }
}
