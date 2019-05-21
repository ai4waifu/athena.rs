//! 内建算子注册表（禁止 engine 层字符串 head 匹配）。

use std::collections::HashMap;

use athena_types::OperatorId;

/// 字符串名 ↔ [`OperatorId`] 双向表。
#[derive(Debug, Clone, Default)]
pub struct OperatorRegistry {
    names: Vec<String>,
    by_name: HashMap<String, OperatorId>,
}

impl OperatorRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 预注册常用内建算子。
    pub fn standard() -> Self {
        let mut reg = Self::new();
        for name in STANDARD_OPERATORS {
            reg.intern(name);
        }
        reg
    }

    /// 分配或查找算子 id。
    pub fn intern(&mut self, name: &str) -> OperatorId {
        if let Some(id) = self.by_name.get(name) {
            return *id;
        }
        let id = OperatorId(self.names.len() as u32);
        self.names.push(name.to_string());
        self.by_name.insert(name.to_string(), id);
        id
    }

    /// 查找已有 id（不分配）。
    pub fn lookup(&self, name: &str) -> Option<OperatorId> {
        self.by_name.get(name).copied()
    }

    /// 反查算子名。
    pub fn name(&self, id: OperatorId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    /// 已注册数量。
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// 求值 / 方言 lowering 常用 head（顺序决定 id，勿改已有顺序）。
const STANDARD_OPERATORS: &[&str] = &[
    "Equal",
    "Less",
    "LessEqual",
    "Greater",
    "GreaterEqual",
    "Unequal",
    "Plus",
    "Times",
    "Power",
    "Subtract",
    "Divide",
    "DotTimes",
    "DotDivide",
    "DotPower",
    "Mldivide",
    "DotLeftDivide",
    "Set",
    "SetDelayed",
    "If",
    "Which",
    "While",
    "For",
    "CompoundExpression",
    "With",
    "Module",
    "Block",
    "Hold",
    "HoldForm",
    "MatchQ",
    "Cases",
    "Table",
    "Sum",
    "Product",
    "Try",
    "Blank",
    "BlankSequence",
    "BlankNullSequence",
    "Pattern",
    "And",
    "Or",
    "Not",
    "Function",
    "Simplify",
    "Sin",
    "Cos",
    "Tan",
    "Sqrt",
    "Abs",
    "Factorial",
    "Log",
    "Exp",
    "Integrate",
    "D",
    "Limit",
    "Map",
    "Part",
    "Span",
    "Det",
    "Transpose",
    "List",
    "Array",
    "Integer",
    "Symbol",
    "String",
    "True",
    "False",
    "Null",
    "Pi",
    "E",
];
