//! 模数上下文（共享预计算句柄；元素可只持 `ModulusId` + residue）。

use std::collections::HashMap;

use athena_types::ModulusId;

use crate::{
    execution_budget::NumericContext,
    kernel::limb as limb_kernel, modular::Modulus, natural::Natural};

/// Montgomery 约化常量（奇模数）。
#[derive(Debug, PartialEq, Eq)]
pub struct MontgomeryParams {
    /// `-m⁻¹ mod 2^64`。
    pub n_prime: u64,
    /// `R² mod m` 的无符号幅度。
    pub r2: Natural,
}

/// Barrett 约化常量（任意 `m > 1`）。
#[derive(Debug, PartialEq, Eq)]
pub struct BarrettParams {
    /// `⌊2^(2k) / m⌋`，`k = ⌈log₂ m⌉`。
    pub mu: Natural,
    /// `k = ⌈log₂ m⌉`。
    pub k: u32,
}

fn power_of_two_natural(bits: u32) -> Natural {
    if bits == 0 {
        return Natural::one();
    }
    let limb_idx = (bits / 64) as usize;
    let bit_in_limb = bits % 64;
    let mut limbs = vec![0u64; limb_idx + 1];
    limbs[limb_idx] = 1u64 << bit_in_limb;
    Natural::from_limbs(limbs).expect("gc numeric alloc")
}

pub(crate) fn montgomery_for_modulus(modulus: &Modulus) -> Option<MontgomeryParams> {
    let mag = modulus.value().magnitude();
    if mag.is_zero() || !mag.is_odd() {
        return None;
    }
    if !limb_kernel::mod_pow_montgomery_eligible(mag.as_limbs()) {
        return None;
    }
    let (n_prime, r2_limbs) = limb_kernel::montgomery_precompute(mag.as_limbs());
    Some(MontgomeryParams { n_prime, r2: Natural::from_limbs(r2_limbs).expect("gc numeric alloc") })
}

pub(crate) fn barrett_for_modulus(modulus: &Modulus) -> BarrettParams {
    let mag = modulus.value().magnitude();
    let k = mag.bits().max(1) as u32;
    let two_2k = power_of_two_natural(2 * k);
    let mu = two_2k.div_rem(&mag).0;
    BarrettParams { mu, k }
}

/// 模运算时间语义。CAS 默认可变时间吞吐；密码学路径须显式要求常数时间。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ModularTimingPolicy {
    /// 可变时间（默认，注重吞吐）。
    #[default]
    VariableTime,
    /// 要求常数时间内核（尚未接线；仅合同标记）。
    ConstantTimeRequired,
}

/// 共享模数上下文（Montgomery/Barrett 常量挂接 [`ModulusTable::intern`]）。
#[derive(Debug, PartialEq, Eq)]
pub struct ModulusContext {
    /// Session 内句柄（intern 后填入）。
    pub id: ModulusId,
    /// 模数（`m > 1`）。
    pub modulus: Modulus,
    /// `⌈log₂(m)⌉`。
    pub bit_length: u64,
    /// 是否奇数（Montgomery 适用性）。
    pub is_odd: bool,
    /// Montgomery 预计算（奇模且足够宽）。
    pub montgomery: Option<MontgomeryParams>,
    /// Barrett 预计算（通用 fallback）。
    pub barrett: Option<BarrettParams>,
    /// 时间语义策略。
    pub timing: ModularTimingPolicy,
}

impl ModulusContext {
    /// 由 [`Modulus`] 构造（未 intern 时 `id` 占位为 `ModulusId(0)`，由 [`ModulusTable`] 覆写）。
    pub fn from_modulus(modulus: Modulus) -> Self {
        let bit_length = modulus.value().bits();
        let is_odd = !modulus.value().rem(&crate::value::integer::Integer::from_i64(2)).expect("two").is_zero();
        let montgomery = if is_odd { montgomery_for_modulus(&modulus) } else { None };
        let barrett = Some(barrett_for_modulus(&modulus));
        Self { id: ModulusId(0), modulus, bit_length, is_odd, montgomery, barrett, timing: ModularTimingPolicy::VariableTime }
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
        let mut ctx = ModulusContext::from_modulus(
            modulus.try_clone_in(&NumericContext::portable_default()).expect("modulus clone for intern"),
        );
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
