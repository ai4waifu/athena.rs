//! 参数化项求值：一次 compile，多次只换局部绑定。
//!
//! 缓存的是程序结构（[`ExecutionModule`]），不是源字符串或局部 `TermId` 全局身份。
//! 定义 / 假设 / 规则集变化时调用方应重新 [`ParameterizedTermPlan::compile`]。

use athena_types::{Result, ResultId, SymbolId, TermId};

use crate::{
    api::request::AthenaRequest,
    execution::{
        LocalBinding, ScopeFrame,
        compiler::ExecutionCompiler,
        ir::{ExecutionModule, ModuleFingerprint},
        vm::{execute_verified_cfg_on_vm_with_frames, materialize_verified_vm_outcome},
    },
    runtime::session::Session,
};

/// 已编译的项计划：可在局部参数绑定下反复执行。
#[derive(Debug, Clone)]
pub struct ParameterizedTermPlan {
    module: ExecutionModule,
}

impl ParameterizedTermPlan {
    /// 将 `term` 编译为可复用 [`ExecutionModule`]（根意图 `EvaluateTerm`）。
    pub fn compile(session: &mut Session, term: TermId) -> Result<Self> {
        let module = ExecutionCompiler::new().compile(session, &AthenaRequest::Term(term))?;
        Ok(Self { module })
    }

    /// 结构指纹（模块身份，非源字符串）。
    pub fn fingerprint(&self) -> ModuleFingerprint {
        self.module.fingerprint
    }

    /// 在单层局部帧中绑定 `locals` 后执行本计划。
    ///
    /// 不修改 Session Own；帧在调用结束时丢弃。
    pub fn execute_with_locals(&self, session: &mut Session, locals: &[(SymbolId, TermId)]) -> Result<ResultId> {
        let mut frame = ScopeFrame::new();
        for &(symbol, term) in locals {
            frame.bind(symbol, LocalBinding::Value(term));
        }
        let mut frames = vec![frame];
        let outcome = execute_verified_cfg_on_vm_with_frames(session, &self.module, &mut frames)?;
        materialize_verified_vm_outcome(session, outcome, "ExecutionIR/parameterized")
    }
}
