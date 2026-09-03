//! 决策变量。

use athena_types::{DomainId, TermId};

use super::ids::VariableId;

/// 变量取值域类别（禁止静默把整数放松为连续）。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum VariableDomain {
    /// 实数。
    Real,
    /// 整数。
    Integer,
    /// 0/1。
    Binary,
    /// 复数（仅允许明确支持的约束族）。
    Complex,
    /// 符号 / 其他域，由 [`DomainId`] 细化。
    Symbolic {
        /// 系数或符号域。
        domain: DomainId,
    },
}

/// 整数性声明（与 [`VariableDomain`] 正交校验）。
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Integrality {
    /// 连续。
    Continuous,
    /// 整数。
    Integer,
    /// 二元。
    Binary,
}

/// 非身份元数据（名称、注释等；不进 fingerprint 主体时可剥离）。
#[derive(Debug, PartialEq, Eq, Default)]
pub struct VariableMetadata {
    /// 展示名（非稳定身份）。
    pub display_name: Option<String>,
}

/// 决策变量。
#[derive(Debug, PartialEq)]
pub struct DecisionVariable {
    /// Session-local id。
    pub id: VariableId,
    /// 取值域。
    pub domain: VariableDomain,
    /// 下界表达式（可选，`None` = −∞ / 无界）。
    pub lower_bound: Option<TermId>,
    /// 上界表达式（可选，`None` = +∞ / 无界）。
    pub upper_bound: Option<TermId>,
    /// 整数性。
    pub integrality: Integrality,
    /// 元数据。
    pub metadata: VariableMetadata,
}

impl DecisionVariable {
    /// 构造连续实变量（骨架便捷路径）。
    pub fn continuous_real(id: VariableId) -> Self {
        Self {
            id,
            domain: VariableDomain::Real,
            lower_bound: None,
            upper_bound: None,
            integrality: Integrality::Continuous,
            metadata: VariableMetadata::default(),
        }
    }

    /// 构造整数变量。
    pub fn integer(id: VariableId) -> Self {
        Self {
            id,
            domain: VariableDomain::Integer,
            lower_bound: None,
            upper_bound: None,
            integrality: Integrality::Integer,
            metadata: VariableMetadata::default(),
        }
    }

    /// 构造二元变量。
    pub fn binary(id: VariableId) -> Self {
        Self {
            id,
            domain: VariableDomain::Binary,
            lower_bound: None,
            upper_bound: None,
            integrality: Integrality::Binary,
            metadata: VariableMetadata::default(),
        }
    }

    /// 整数性与域是否一致（不一致必须拒收，不得静默放松）。
    pub fn integrality_consistent(&self) -> bool {
        match (self.domain, self.integrality) {
            (VariableDomain::Real | VariableDomain::Complex | VariableDomain::Symbolic { .. }, Integrality::Continuous) => true,
            (VariableDomain::Integer, Integrality::Integer) => true,
            (VariableDomain::Binary, Integrality::Binary) => true,
            _ => false,
        }
    }
}
