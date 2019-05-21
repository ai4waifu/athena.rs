//! 绑定值表：在完整 IR intern 接通前，把 [`ExprId`] 句柄映射到标量解。

use std::collections::BTreeMap;

use athena_numeric::{Number, Rational};
use athena_types::ExprId;

/// 解绑定标量（adapter 产出，非方言 AST）。
#[derive(Debug, PartialEq)]
pub enum BindingValue {
    /// 精确有理。
    Rational(Rational),
    /// 机器浮点。
    MachineF64(f64),
    /// 通用内核数（预留）。
    Number(Number),
}

/// [`ExprId`] → 标量值的局部表（adapter 私有 arena 替身）。
#[derive(Debug, PartialEq, Default)]
pub struct BindingValueTable {
    /// 有序表。
    pub values: BTreeMap<ExprId, BindingValue>,
    next_id: u32,
}

impl BindingValueTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 分配新 [`ExprId`] 并记录值。
    pub fn intern(&mut self, value: BindingValue) -> ExprId {
        let id = ExprId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.values.insert(id, value);
        id
    }

    /// 查询。
    pub fn get(&self, id: ExprId) -> Option<&BindingValue> {
        self.values.get(&id)
    }
}
