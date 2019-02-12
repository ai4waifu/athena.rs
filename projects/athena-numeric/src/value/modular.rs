//! 模数与模整数（numeric 层；公共面为 [`Integer`]，不暴露 `num_bigint`）。
//!
//! 布局：`residue` / 嵌入 `modulus` 均为自有 `meta + Magnitude`（sign 忽略；恒非负）。
//! 禁止再套一层完整 [`Integer`] 存储字段。

use athena_types::{Diagnostic, DiagnosticCode, ModulusId, Result};

use crate::{integer::Integer, modulus_context::ModulusTable, storage::MagnitudePair};

/// 正整数模数（`m > 1`）。
#[derive(Clone)]
pub struct Modulus {
    value: MagnitudePair,
}

impl core::fmt::Debug for Modulus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Modulus").field("value", &self.value()).finish()
    }
}

impl PartialEq for Modulus {
    fn eq(&self, other: &Self) -> bool {
        self.value.as_limbs() == other.value.as_limbs()
    }
}

impl Eq for Modulus {}

impl core::hash::Hash for Modulus {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.value.as_limbs().hash(state);
    }
}

impl Modulus {
    /// 由精确整数构造；`m ≤ 1` → `ATHENA_MODULUS_INVALID`。
    pub fn new(value: impl Into<Integer>) -> Result<Self> {
        let value = value.into();
        if !value.is_positive() || value.is_one() {
            return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid).detail("value", value.to_decimal_string()));
        }
        Ok(Self { value: value.into_pair().with_negative(false) })
    }

    /// 模数值（始终 `> 1`）。
    pub fn value(&self) -> Integer {
        Integer::from_pair(self.value.clone())
    }

    /// 将整数规范到 `[0, m)`。
    pub fn reduce(&self, n: &Integer) -> Integer {
        n.rem_euclid(&self.value()).expect("modulus > 1")
    }
}

/// 模数绑定：嵌入完整 [`Modulus`] 或 session 内 [`ModulusId`]。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModulusBinding {
    /// 自包含模数（无 intern 表时）。
    Embedded(Modulus),
    /// Session intern 句柄（经 [`ModulusTable`] 解析）。
    Interned(ModulusId),
}

/// 绑定模数的剩余类代表。
#[derive(Clone)]
pub struct ModularValue {
    /// 规范剩余 `[0, modulus)`（unsigned Magnitude）。
    residue: MagnitudePair,
    /// 模数绑定。
    binding: ModulusBinding,
}

impl core::fmt::Debug for ModularValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ModularValue").field("residue", &self.residue()).field("binding", &self.binding).finish()
    }
}

impl PartialEq for ModularValue {
    fn eq(&self, other: &Self) -> bool {
        self.residue.as_limbs() == other.residue.as_limbs() && self.binding == other.binding
    }
}

impl Eq for ModularValue {}

impl ModularValue {
    /// 在给定模数下构造（自动化约，嵌入模数）。
    pub fn new(residue: impl Into<Integer>, modulus: Modulus) -> Self {
        let residue = modulus.reduce(&residue.into());
        Self { residue: residue.into_pair().with_negative(false), binding: ModulusBinding::Embedded(modulus) }
    }

    /// 用已 intern 的 [`ModulusId`] 构造（剩余须已约化或由 caller 保证）。
    pub fn new_interned(residue: Integer, modulus_id: ModulusId) -> Self {
        Self { residue: residue.into_pair().with_negative(false), binding: ModulusBinding::Interned(modulus_id) }
    }

    /// 剩余。
    pub fn residue(&self) -> Integer {
        Integer::from_pair(self.residue.clone())
    }

    /// 嵌入模数（仅 `Embedded` 绑定）。
    pub fn modulus(&self) -> Option<&Modulus> {
        match &self.binding {
            ModulusBinding::Embedded(m) => Some(m),
            ModulusBinding::Interned(_) => None,
        }
    }

    /// Session 模数句柄（仅 `Interned` 绑定）。
    pub fn modulus_id(&self) -> Option<ModulusId> {
        match &self.binding {
            ModulusBinding::Interned(id) => Some(*id),
            ModulusBinding::Embedded(_) => None,
        }
    }

    /// 经 intern 表解析模数（嵌入绑定则克隆）。
    pub fn resolve_modulus(&self, table: &ModulusTable) -> Result<Modulus> {
        match &self.binding {
            ModulusBinding::Embedded(m) => Ok(m.clone()),
            ModulusBinding::Interned(id) => table
                .get(*id)
                .map(|ctx| ctx.modulus.clone())
                .ok_or_else(|| Diagnostic::new(DiagnosticCode::DomainMismatch).detail("reason", "unknown ModulusId")),
        }
    }

    /// 同模运算前置检查（嵌入模数直接比；intern 需相同 id）。
    pub fn same_modulus(&self, other: &Self) -> Result<()> {
        match (&self.binding, &other.binding) {
            (ModulusBinding::Embedded(l), ModulusBinding::Embedded(r)) if l == r => Ok(()),
            (ModulusBinding::Interned(l), ModulusBinding::Interned(r)) if l == r => Ok(()),
            _ => Err(Diagnostic::new(DiagnosticCode::DomainMismatch).detail("operation", "modular_binop")),
        }
    }

    /// 同模检查（可解析 intern 绑定）。
    pub fn same_modulus_with_table(&self, other: &Self, table: &ModulusTable) -> Result<()> {
        let l = self.resolve_modulus(table)?;
        let r = other.resolve_modulus(table)?;
        if l == r { Ok(()) } else { Err(Diagnostic::new(DiagnosticCode::DomainMismatch).detail("operation", "modular_binop")) }
    }
}

/// 已证明为素数的模数（构造前须由 engine 完成确定性素性判定）。
///
/// 仅 [`PrimeModulus`] 可构造 exact `F_p`。numeric 层不自证素性。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrimeModulus {
    inner: Modulus,
}

impl PrimeModulus {
    /// 在 caller 已确立 **确定素数** 后构造（engine 层负责校验）。
    pub fn assuming_proven(value: impl Into<Integer>) -> Result<Self> {
        Ok(Self { inner: Modulus::new(value)? })
    }

    /// 底层 [`Modulus`]。
    pub fn modulus(&self) -> &Modulus {
        &self.inner
    }

    /// 素数 `p`。
    pub fn value(&self) -> Integer {
        self.inner.value()
    }
}

/// 概率素数模数（仅允许概率语义路径，不得构造 exact `F_p`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProbablePrimeModulus {
    inner: Modulus,
}

impl ProbablePrimeModulus {
    /// 在 caller 已记录概率素性证据后构造。
    pub fn assuming_probable(value: impl Into<Integer>) -> Result<Self> {
        Ok(Self { inner: Modulus::new(value)? })
    }

    /// 底层 [`Modulus`]。
    pub fn modulus(&self) -> &Modulus {
        &self.inner
    }

    /// 模数值。
    pub fn value(&self) -> Integer {
        self.inner.value()
    }
}
