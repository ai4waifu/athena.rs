//! 类型化 SSA 操作（封闭操作码族 — 无字符串 handler 查找）。

use athena_ir::SemanticOperator;
use athena_types::{BindingEvaluationPolicy, BindingKind, CollectionKind, CompiledRuleId, DispatchTableId, ExtensionOperatorId, IndexSpec};

use super::{
    ids::{CapturedRootId, ConstantId, EffectToken, InputId, ProviderCallId, SsaValueId},
    types::ExecutionValueType,
};

/// 一条 SSA 操作定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// 本操作定义的结果值（仅 unit 的操作为 `None`）。
    pub result: Option<SsaValueId>,
    /// `result` 存在时的静态结果类型。
    pub result_type: ExecutionValueType,
    /// 操作码载荷。
    pub kind: OperationKind,
    /// 有副作用的操作码所需的入边 effect token。
    pub effect_in: Option<EffectToken>,
    /// 有副作用的操作码产生的 effect token。
    pub effect_out: Option<EffectToken>,
}

/// `ExecutionIR` 的封闭语义操作码集合。
///
/// 方言表层名称不得出现于此。仅封闭语义算子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    /// 将 module 输入加载为 SSA 值。
    LoadInput {
        /// 输入表下标。
        input: InputId,
    },
    /// 加载不可变项根（只读句柄）。
    LoadTerm {
        /// 捕获的项根。
        root: CapturedRootId,
    },
    /// 物化 module 常量。
    Constant {
        /// 常量表下标。
        constant: ConstantId,
    },
    /// 对 SSA 实参应用封闭语义算子。
    ApplySemanticOperator {
        /// 封闭语义算子标识。
        operator: athena_ir::SemanticOperator,
        /// 操作数 SSA 值。
        args: Vec<SsaValueId>,
    },
    /// 应用扩展算子（显示名经注册表 — 非核心数学）。
    ApplyExtensionOperator {
        /// 扩展标识。
        operator: ExtensionOperatorId,
        /// 操作数 SSA 值。
        args: Vec<SsaValueId>,
    },
    /// 由已求值的元素 SSA 值构造类型化集合。
    ConstructCollection {
        /// 集合种类（绝非隐式方言 `List`）。
        kind: CollectionKind,
        /// 元素 SSA 值（保持顺序）。
        elements: Vec<SsaValueId>,
    },
    /// 对值 / 集合做下标访问。
    Index {
        /// 目标 SSA 值。
        target: SsaValueId,
        /// 各轴下标规格。
        axes: Vec<IndexSpec>,
    },
    /// 读取 Session / 作用域绑定。
    ReadBinding {
        /// 绑定键 SSA 值（符号 / 槽句柄）。
        key: SsaValueId,
    },
    /// 写入 Session / 作用域绑定。
    WriteBinding {
        /// 绑定键。
        key: SsaValueId,
        /// 写入的值。
        value: SsaValueId,
        /// 绑定类别。
        kind: BindingKind,
        /// 求值策略。
        evaluation: BindingEvaluationPolicy,
    },
    /// 在头绑定上注册 pattern → replacement 分派规则。
    RegisterRuleDispatch {
        /// 头符号键（所有权 / 清除）。
        head: SsaValueId,
        /// 编译期封闭的扩展算子（执行时不 intern 显示名）。
        operator: ExtensionOperatorId,
        /// Pattern 项（lowering 时已中立化 / 编译）。
        pattern: SsaValueId,
        /// 替换模板项。
        replacement: SsaValueId,
    },
    /// 将预编译规则挂到分派表（`SessionCommand::RegisterRuleDispatch`）。
    RegisterCompiledRule {
        /// 目标分派表。
        table: DispatchTableId,
        /// 预编译规则句柄。
        rule: CompiledRuleId,
    },
    /// 进入词法或动态作用域帧。
    EnterScope {
        /// 可选的父作用域 SSA 句柄。
        parent: Option<SsaValueId>,
    },
    /// 退出当前作用域帧。
    ExitScope {
        /// 由 [`Self::EnterScope`] 产生的作用域句柄。
        scope: SsaValueId,
    },
    /// 类型化 provider 调用。
    CallProvider {
        /// 描述符表下标。
        call: ProviderCallId,
        /// 实参 SSA 值。
        args: Vec<SsaValueId>,
    },
    /// 可能走显式出口边的 guard。
    Guard {
        /// 谓词 SSA 值（类型化 Boolean）。
        predicate: SsaValueId,
        /// 成功则块内继续；失败走终结器 / 出口表。
        on_failure: GuardFailure,
    },
    /// 从 SSA / 项句柄物化运行时 `Value`。
    MaterializeValue {
        /// 源 SSA 值。
        source: SsaValueId,
    },
    /// 发布到 `ResultStore`。
    PublishResult {
        /// 要发布的值或残差。
        source: SsaValueId,
    },
}

/// Guard 失败路由（成功则 fall-through）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardFailure {
    /// 路由到已声明的 module 出口。
    Exit(super::ids::ExitId),
    /// 经终结器合同立即拒绝当前 region。
    Reject,
}

impl Operation {
    /// 纯常量加载。
    pub fn constant(result: SsaValueId, constant: ConstantId) -> Self {
        Self {
            result: Some(result),
            result_type: ExecutionValueType::Unknown,
            kind: OperationKind::Constant { constant },
            effect_in: None,
            effect_out: None,
        }
    }

    /// 纯项加载。
    pub fn load_term(result: SsaValueId, root: CapturedRootId) -> Self {
        Self {
            result: Some(result),
            result_type: ExecutionValueType::Term,
            kind: OperationKind::LoadTerm { root },
            effect_in: None,
            effect_out: None,
        }
    }
}
