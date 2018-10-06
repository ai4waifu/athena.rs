//! 多项式环 intern 表（Session 级内容寻址）。

use std::collections::HashMap;

use athena_types::{Diagnostic, RingId, SymbolId};

use super::{
    order::MonomialOrder,
    ring::{CoefficientDomain, RingDescriptor},
};

/// 环 intern 键（特征由系数域推导，不单独进 key）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RingInternKey {
    coefficients: CoefficientDomain,
    variables: Vec<SymbolId>,
    order: MonomialOrder,
}

/// Session 持有的多项式环注册表。
#[derive(Debug, Default)]
pub struct RingTable {
    next_id: u32,
    by_id: HashMap<RingId, RingDescriptor>,
    by_key: HashMap<RingInternKey, RingId>,
}

impl RingTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 内容寻址 intern；相同 `(coefficients, variables, order)` 返回同一 [`RingId`]。
    pub fn intern(
        &mut self,
        coefficients: CoefficientDomain,
        variables: Vec<SymbolId>,
        order: MonomialOrder,
    ) -> Result<RingId, Diagnostic> {
        let (coefficients, variables, order, characteristic) =
            RingDescriptor::validate_content(coefficients, variables, order)?;
        let key = RingInternKey { coefficients: coefficients.clone(), variables: variables.clone(), order: order.clone() };
        if let Some(&id) = self.by_key.get(&key) {
            return Ok(id);
        }
        let id = RingId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let desc = RingDescriptor::with_id(id, coefficients, variables, order, characteristic);
        self.by_key.insert(key, id);
        self.by_id.insert(id, desc);
        Ok(id)
    }

    /// 按 id 查描述符。
    pub fn get(&self, id: RingId) -> Option<&RingDescriptor> {
        self.by_id.get(&id)
    }

    /// 已注册环数量。
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}
