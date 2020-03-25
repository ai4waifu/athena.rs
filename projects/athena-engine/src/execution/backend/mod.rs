//! 后端合同 — 原生 JIT、WASM 与领域 kernel 消费同一 `ExecutionIR`。

use athena_types::{Diagnostic, DiagnosticCode, Result, ResultId};

use crate::{
    execution::{ir::ExecutionModule, reference::ReferenceExecutor},
    runtime::session::Session,
};

/// 用于代码缓存键的能力 / ABI 指纹（不是 `TermId` 下标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendAbiFingerprint(pub u64);

impl BackendAbiFingerprint {
    /// 由已校验 module 指纹与后端种类推导缓存键。
    pub fn of_module(module: &ExecutionModule, kind: BackendKind) -> Self {
        use std::{
            collections::hash_map::DefaultHasher,
            hash::{Hash, Hasher},
        };
        let mut hasher = DefaultHasher::new();
        0x4245_4142_4946_5052u64.hash(&mut hasher); // "BEABIFPR"
        module.fingerprint.0.hash(&mut hasher);
        core::mem::discriminant(&kind).hash(&mut hasher);
        Self(hasher.finish())
    }
}

/// 所选可执行后端。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// 正确性 / 回放预言机。
    Reference,
    /// 可选原生 JIT。
    NativeJit,
    /// 可选 WASM。
    Wasm,
    /// Provider 私有的纯 region kernel 产物。
    DomainKernel,
}

/// 所有后端共享的入口合同。
pub trait ExecutionBackend {
    /// 后端分类。
    fn kind(&self) -> BackendKind;

    /// 执行已校验 module。不支持的路径须返回类型化诊断
    /// — 绝不可静默回退到另一套执行模型。
    fn execute(&self, session: &mut Session, module: &ExecutionModule) -> Result<ResultId>;
}

impl ExecutionBackend for ReferenceExecutor {
    fn kind(&self) -> BackendKind {
        BackendKind::Reference
    }

    fn execute(&self, session: &mut Session, module: &ExecutionModule) -> Result<ResultId> {
        ReferenceExecutor::execute(self, session, module, None)
    }
}

/// 占位原生 JIT 后端（未接线）。
#[derive(Debug, Default)]
pub struct NativeJitBackend {}

impl ExecutionBackend for NativeJitBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::NativeJit
    }

    fn execute(&self, _session: &mut Session, _module: &ExecutionModule) -> Result<ResultId> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "NativeJitBackend")
            .detail("status", "contract_frozen_not_wired"))
    }
}

/// 占位 WASM 后端（未接线）。
#[derive(Debug, Default)]
pub struct WasmBackend {}

impl ExecutionBackend for WasmBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Wasm
    }

    fn execute(&self, _session: &mut Session, _module: &ExecutionModule) -> Result<ResultId> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "WasmBackend")
            .detail("status", "contract_frozen_not_wired"))
    }
}

/// 占位领域 kernel 后端（私有产物仅经 `CallProvider`）。
#[derive(Debug, Default)]
pub struct DomainKernelBackend {}

impl ExecutionBackend for DomainKernelBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::DomainKernel
    }

    fn execute(&self, _session: &mut Session, _module: &ExecutionModule) -> Result<ResultId> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "DomainKernelBackend")
            .detail("status", "contract_frozen_not_wired"))
    }
}
