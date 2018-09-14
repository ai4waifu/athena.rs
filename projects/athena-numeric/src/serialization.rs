//! 数值序列化 wire（骨架）。

use athena_types::{NumericKind, SerializationVersion};

use crate::precision::PrecisionInfo;

/// 跨进程 / arena 稳定数值载荷。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericValueWire {
    /// 种类。
    pub kind: NumericKind,
    /// 域描述字节（骨架）。
    pub domain_payload: Vec<u8>,
    /// 值载荷。
    pub payload: Vec<u8>,
    /// 精度。
    pub precision: PrecisionInfo,
    /// schema 版本。
    pub version: SerializationVersion,
}

impl NumericValueWire {
    /// 当前 schema。
    pub fn current_version() -> SerializationVersion {
        SerializationVersion::CURRENT
    }
}
