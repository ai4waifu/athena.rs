//! 可证明性质状态（对齐 M-Graph determinacy）。

/// 性质见证（witness id / 证书载荷待扩展）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct PropertyWitness {
    /// 见证描述（算法名或 witness 句柄字符串；非数学对象身份）。
    pub kind: String,
}

impl PropertyWitness {
    /// 构造占位见证。
    pub fn placeholder(kind: impl Into<String>) -> Self {
        Self { kind: kind.into() }
    }

    /// Owning 复制（Living `31`：描述字符串）。
    pub fn owning_copy(&self) -> Self {
        Self { kind: self.kind.clone() }
    }
}

/// 代数性质：已知 / 否证 / 或然 / 未知 / 资源截断。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]（`T: Copy`）
/// 或 [`Self::owning_copy_with`]。
#[derive(Debug, PartialEq)]
pub enum PropertyState<T> {
    /// 已证明成立。
    Proven {
        /// 性质值。
        value: T,
        /// 见证。
        witness: PropertyWitness,
    },
    /// 已证明不成立。
    Disproven {
        /// 否证见证。
        witness: PropertyWitness,
    },
    /// 启发式或概率性结论。
    Probable {
        /// 候选值。
        value: T,
        /// 置信度 0..=1。
        confidence: f64,
        /// 方法名。
        method: String,
    },
    /// 尚未判定。
    Unknown,
    /// 资源耗尽下的部分信息。
    ResourceLimited {
        /// 部分值（若有）。
        partial: Option<T>,
    },
}

impl<T> PropertyState<T> {
    /// 是否已有确定真值。
    pub fn is_proven(&self) -> bool {
        matches!(self, Self::Proven { .. })
    }

    /// 已证明的值（若有）。
    pub fn proven_value(&self) -> Option<&T> {
        match self {
            Self::Proven { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Owning 复制（Living `31`：值经 `copy_value`）。
    pub fn owning_copy_with(&self, copy_value: impl Fn(&T) -> T) -> Self {
        match self {
            Self::Proven { value, witness } => Self::Proven {
                value: copy_value(value),
                witness: witness.owning_copy(),
            },
            Self::Disproven { witness } => Self::Disproven {
                witness: witness.owning_copy(),
            },
            Self::Probable { value, confidence, method } => Self::Probable {
                value: copy_value(value),
                confidence: *confidence,
                method: method.clone(),
            },
            Self::Unknown => Self::Unknown,
            Self::ResourceLimited { partial } => Self::ResourceLimited {
                partial: partial.as_ref().map(&copy_value),
            },
        }
    }
}

impl<T: Copy> PropertyState<T> {
    /// Owning 复制（Living `31`：`T` 为 Copy 载荷）。
    pub fn owning_copy(&self) -> Self {
        self.owning_copy_with(|v| *v)
    }
}

impl<T> Default for PropertyState<T> {
    fn default() -> Self {
        Self::Unknown
    }
}
