//! 与源无关的 module 指纹（不是 `TermId` 下标或渲染文本）。

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use super::module::ExecutionModule;

/// 对 module 结构内容的稳定指纹。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleFingerprint(pub u64);

impl ModuleFingerprint {
    /// 计算结构指纹。
    ///
    /// 使用专用种子，避免默认 hasher 跨进程不稳定被静默当作缓存键。
    /// 需要跨进程稳定性的调用方，日后须换成固定字节的规范编码。
    pub fn of_module(module: &ExecutionModule) -> Self {
        let mut hasher = DefaultHasher::new();
        0x4154_4845_4e41_4558u64.hash(&mut hasher); // "ATHENAEX"
        module.inputs.len().hash(&mut hasher);
        module.constants.len().hash(&mut hasher);
        module.captured_roots.len().hash(&mut hasher);
        module.regions.len().hash(&mut hasher);
        module.effect_edges.len().hash(&mut hasher);
        module.exits.len().hash(&mut hasher);
        module.provider_calls.len().hash(&mut hasher);
        for edge in &module.effect_edges {
            edge.token.0.hash(&mut hasher);
            edge.precedes_from.map(|t| t.0).hash(&mut hasher);
            core::mem::discriminant(&edge.kind).hash(&mut hasher);
        }
        for exit in &module.exits {
            exit.id.0.hash(&mut hasher);
            core::mem::discriminant(&exit.kind).hash(&mut hasher);
        }
        for region in &module.regions {
            region.id.0.hash(&mut hasher);
            region.entry.0.hash(&mut hasher);
            region.blocks.len().hash(&mut hasher);
            for block in &region.blocks {
                block.id.0.hash(&mut hasher);
                block.parameters.len().hash(&mut hasher);
                block.operations.len().hash(&mut hasher);
                for op in &block.operations {
                    core::mem::discriminant(&op.kind).hash(&mut hasher);
                    op.effect_in.map(|t| t.0).hash(&mut hasher);
                    op.effect_out.map(|t| t.0).hash(&mut hasher);
                }
            }
        }
        Self(hasher.finish())
    }
}
