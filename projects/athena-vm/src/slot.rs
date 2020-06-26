//! 执行期槽值与稠密槽表。
//!
//! 槽只持有 typed 句柄，**不**拥有 `TermStore` / DomainObject / GC payload。

#![allow(unsafe_code)]

use athena_types::{ResultId, SymbolId, TermId, ValueId};

/// SSA / 局部槽中的运行时值（句柄闭集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotValue {
    /// 未写入（稠密表空洞）。
    #[default]
    Empty,
    /// 类型化布尔值。
    Boolean(bool),
    /// `TermStore` 句柄。
    Term(TermId),
    /// 绑定键。
    Symbol(SymbolId),
    /// `ValueStore` 句柄。
    Value(ValueId),
    /// 已物化计算结果句柄。
    Result(ResultId),
    /// 作用域帧深度句柄（来自 `EnterScope`）。
    Scope(u32),
    /// Unit（空单元值）。
    Unit,
}

impl SlotValue {
    /// 是否为未写入槽。
    #[inline]
    pub const fn is_empty(self) -> bool {
        matches!(self, Self::Empty)
    }
}

/// 按 `u32` 稠密索引的槽表（热路径可走未检查下标）。
#[derive(Debug, Clone, Default)]
pub struct SlotTable {
    slots: Vec<SlotValue>,
}

impl SlotTable {
    /// 空表。
    #[inline]
    pub const fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// 预分配容量（元素仍为 [`SlotValue::Empty`]）。
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        slots.resize(capacity, SlotValue::Empty);
        Self { slots }
    }

    /// 当前物理长度。
    #[inline]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// 是否无槽。
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// 确保至少 `len` 个槽（不足则填 [`SlotValue::Empty`]）。
    #[inline]
    pub fn ensure(&mut self, len: u32) {
        let need = len as usize;
        if self.slots.len() < need {
            self.slots.resize(need, SlotValue::Empty);
        }
    }

    /// 读取已定义槽；越界或 [`SlotValue::Empty`] 返回 `None`。
    #[inline]
    pub fn get(&self, index: u32) -> Option<SlotValue> {
        self.slots.get(index as usize).copied().filter(|s| !s.is_empty())
    }

    /// 写入槽（必要时扩容）。
    #[inline]
    pub fn set(&mut self, index: u32, value: SlotValue) {
        let i = index as usize;
        if i >= self.slots.len() {
            self.slots.resize(i + 1, SlotValue::Empty);
        }
        self.slots[i] = value;
    }

    /// 清除槽为 [`SlotValue::Empty`]（越界为空操作）。
    #[inline]
    pub fn clear_at(&mut self, index: u32) {
        if let Some(slot) = self.slots.get_mut(index as usize) {
            *slot = SlotValue::Empty;
        }
    }

    /// 热路径读取。
    ///
    /// # Safety
    /// `index` 必须 `< self.len()`。
    #[inline]
    pub unsafe fn get_unchecked(&self, index: u32) -> SlotValue {
        unsafe { *self.slots.get_unchecked(index as usize) }
    }

    /// 热路径写入。
    ///
    /// # Safety
    /// `index` 必须 `< self.len()`。
    #[inline]
    pub unsafe fn set_unchecked(&mut self, index: u32, value: SlotValue) {
        unsafe {
            *self.slots.get_unchecked_mut(index as usize) = value;
        }
    }
}
