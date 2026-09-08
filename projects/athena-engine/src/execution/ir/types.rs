//! `ExecutionIR` 内 SSA 值携带的静态类型。

use athena_types::{ExtensionOperatorId, ResultId, SymbolId, TermId, TermRef, ValueId};

use super::ids::{CapturedRootId, ConstantId, InputId, ProviderCallId};
use crate::domains::DomainPayloadId;

/// SSA 值的封闭值类型格。
///
/// 其他 Athena 域的标识仅以类型化句柄出现，绝不与
/// [`super::ids::SsaValueId`] 共用 id 命名空间。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionValueType {
    /// 未类型化 / 尚未约束（最终 module 中校验器会拒绝）。
    Unknown,
    /// 类型化 Boolean。
    Boolean,
    /// 符号绑定键（`SymbolId` 句柄，不是 Term）。
    Symbol,
    /// 符号项句柄（`TermStore` 标识）。
    Term,
    /// 运行时值句柄（`ValueStore` 标识）。
    Value,
    /// 已发布计算结果句柄。
    Result,
    /// 不透明 provider 载荷句柄。
    ProviderPayload,
    /// 运行时作用域帧句柄（来自 `EnterScope`）。
    Scope,
    /// Unit / void（仅副作用的操作）。
    Unit,
}

/// Module 级常量载荷（编译期已知）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstantValue {
    /// Boolean 字面量。
    Boolean(bool),
    /// 绑定键符号。
    Symbol(SymbolId),
    /// 已存在于 `TermStore` 的 intern 项根。
    Term(TermId),
    /// 已存在于 `ValueStore` 的运行时值句柄（含矩阵 DomainObject）。
    Value(ValueId),
    /// Unit 常量。
    Unit,
}

/// Module 输入绑定（请求 / 快照边）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInput {
    /// 稳定输入槽。
    pub id: InputId,
    /// 输入 SSA 值的静态类型。
    pub ty: ExecutionValueType,
}

/// Module 引用的捕获 GC / Session 根（非 IR 拥有）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapturedRoot {
    /// TermStore 节点根（带 store epoch / generation）。
    Term(TermRef),
    /// ValueStore 对象根。
    Value(ValueId),
    /// ResultStore 条目根。
    Result(ResultId),
}

/// 类型化 provider 调用点的描述符（语言中立）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCallDescriptor {
    /// 描述符表下标。
    pub id: ProviderCallId,
    /// 封闭语义算子标识（不是方言表层名）。
    pub operator: ExtensionOperatorId,
    /// 期望的实参类型。
    pub argument_types: Vec<ExecutionValueType>,
    /// 结果类型。
    pub result_type: ExecutionValueType,
    /// 该调用是否为 GC / 预算 / 取消 safepoint。
    pub safepoint: bool,
    /// Session [`DomainPayloadId`]：Goal→`CallProvider` 的自包含载荷绑定。
    ///
    /// **禁止**再经 host `pending_domain` 侧通道注入。
    pub payload: Option<DomainPayloadId>,
}

/// Module 表的便捷构造。
impl ConstantValue {
    /// Boolean 常量。
    pub fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    /// Symbol 常量。
    pub fn symbol(symbol: SymbolId) -> Self {
        Self::Symbol(symbol)
    }

    /// Term 常量。
    pub fn term(term: TermId) -> Self {
        Self::Term(term)
    }

    /// ValueStore 句柄常量。
    pub fn value(value: ValueId) -> Self {
        Self::Value(value)
    }
}

impl ModuleInput {
    /// 类型化 module 输入。
    pub fn new(id: InputId, ty: ExecutionValueType) -> Self {
        Self { id, ty }
    }
}

impl CapturedRoot {
    /// 包装带 generation 的项根。
    pub fn term(term: TermRef) -> Self {
        Self::Term(term)
    }
}

impl ProviderCallDescriptor {
    /// 最小 provider 描述符（无领域载荷）。
    pub fn new(id: ProviderCallId, operator: ExtensionOperatorId, result_type: ExecutionValueType) -> Self {
        Self { id, operator, argument_types: Vec::new(), result_type, safepoint: true, payload: None }
    }

    /// Goal 路径：绑定 Session 领域载荷句柄。
    pub fn with_domain_payload(mut self, payload: DomainPayloadId) -> Self {
        self.payload = Some(payload);
        self
    }
}

/// 冻结测试 / 构建器用的表 id 解析。
pub fn unused_constant_id() -> ConstantId {
    ConstantId(0)
}

/// 冻结测试 / 构建器用的捕获根表 id 解析。
pub fn unused_captured_root_id() -> CapturedRootId {
    CapturedRootId(0)
}
