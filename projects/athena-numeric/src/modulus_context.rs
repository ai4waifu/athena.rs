//! 模数上下文（共享预计算句柄；元素可只持 `ModulusId` + residue）。

use std::collections::HashMap;

use athena_types::ModulusId;

use crate::modular::Modulus;

/// 模运算时间语义。CAS 默认可变时间吞吐；密码学路径须显式要求常数时间。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ModularTimingPolicy {
    /// 可变时间（默认，注重吞吐）。
    #[default]
    VariableTime,
    /// 要求常数时间内核（尚未接线；仅合同标记）。
    ConstantTimeRequired,
}

/// 共享模数上下文（Montgomery/Barrett 常量后续挂接）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulusContext {
    /// Session 内句柄（intern 后填入）。
    pub id: ModulusId,
    /// 模数（`m > 1`）。
    pub modulus: Modulus,
    /// `⌈log₂(m)⌉`。
    pub bit_length: u64,
    /// 是否奇数（Montgomery 适用性）。
    pub is_odd: bool,
    /// 时间语义策略。
    pub timing: ModularTimingPolicy,
}

impl ModulusContext {
    /// 由 [`Modulus`] 构造（未 intern 时 `id` 占位为 `ModulusId(0)`，由 [`ModulusTable`] 覆写）。
    pub fn from_modulus(modulus: Modulus) -> Self {
        let bit_length = modulus.value().bits();
        let is_odd = !modulus.value().rem(&crate::integer::Integer::from_i64(2)).is_zero();
        Self {
            id: ModulusId(0),
            modulus,
            bit_length,
            is_odd,
            timing: ModularTimingPolicy::VariableTime,
        }
    }
}

/// Session 级模数 intern 表（内容寻址）。
#[derive(Debug, Default)]
pub struct ModulusTable {
    next_id: u32,
    by_modulus: HashMap<Modulus, ModulusId>,
    contexts: HashMap<ModulusId, ModulusContext>,
}

impl ModulusTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern 模数，返回稳定 [`ModulusId`]（同模幂等）。
    pub fn intern(&mut self, modulus: Modulus) -> ModulusId {
        if let Some(&id) = self.by_modulus.get(&modulus) {
            return id;
        }
        let id = ModulusId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let mut ctx = ModulusContext::from_modulus(modulus.clone());
        ctx.id = id;
        self.by_modulus.insert(modulus, id);
        self.contexts.insert(id, ctx);
        id
    }

    /// 按 id 查上下文。
    pub fn get(&self, id: ModulusId) -> Option<&ModulusContext> {
        self.contexts.get(&id)
    }

    /// 已注册数量。
    pub fn len(&self) -> usize {
        self.contexts.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }
}
