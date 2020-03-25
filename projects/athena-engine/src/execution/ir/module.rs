//! 顶层 `ExecutionModule` — 唯一的可执行 IR 单元。

use super::{
    effect::EffectEdge,
    exit::DeclaredExit,
    fingerprint::ModuleFingerprint,
    ids::RegionId,
    region::Region,
    types::{CapturedRoot, ConstantValue, ModuleInput, ProviderCallDescriptor},
};

/// 由 [`crate::execution::compiler::ExecutionCompiler`] 产出的已校验可执行 module。
///
/// 这不是 AST、字节码流或任务队列。后端消费同一 module：
/// reference 执行器、原生 JIT、WASM，以及经 `CallProvider` 的领域 kernel。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionModule {
    /// 请求 / 快照输入。
    pub inputs: Vec<ModuleInput>,
    /// 编译期常量。
    pub constants: Vec<ConstantValue>,
    /// 捕获的运行时根（IR 不拥有它们）。
    pub captured_roots: Vec<CapturedRoot>,
    /// 控制流 region。
    pub regions: Vec<Region>,
    /// 有序 effect 签名。
    pub effect_edges: Vec<EffectEdge>,
    /// Guard / failure / deoptimization 出口。
    pub exits: Vec<DeclaredExit>,
    /// 类型化 provider 调用描述符。
    pub provider_calls: Vec<ProviderCallDescriptor>,
    /// 与源无关的结构指纹。
    pub fingerprint: ModuleFingerprint,
}

impl ExecutionModule {
    /// 带单个空入口 region 的空 module（合同冻结占位）。
    pub fn empty() -> Self {
        use super::{block::BasicBlock, ids::BlockId};

        let entry = BasicBlock::empty_return(BlockId(0));
        let region = Region::from_entry_block(RegionId(0), entry, Vec::new());
        let mut module = Self {
            inputs: Vec::new(),
            constants: Vec::new(),
            captured_roots: Vec::new(),
            regions: vec![region],
            effect_edges: Vec::new(),
            exits: Vec::new(),
            provider_calls: Vec::new(),
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        module
    }

    /// 入口 region id（若存在）。
    pub fn entry_region(&self) -> Option<RegionId> {
        self.regions.first().map(|r| r.id)
    }
}

/// 设计叙述中的公开别名（`ExecutionIR` ≡ 已校验的 [`ExecutionModule`] 图）。
pub type ExecutionIR = ExecutionModule;
