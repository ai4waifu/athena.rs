//! `ReferenceExecutor` — 过渡期 **host adapter** 名（暂住 engine）。
//!
//! 解释循环只走 [`crate::execution::vm::execute_verified_cfg_on_vm_with_config`]。
//! 本模块**不再**保留 `eval_region` / 第二套 CFG 语义循环。语义经
//! [`crate::execution::execution_host::ExecutionHost`]（`VmHost`）。

mod helpers;

pub(crate) use self::helpers::{
    CompareOutcome, IndexOutcome, domain_result_symbolic_term, evaluate_arithmetic_terms, evaluate_compare_terms,
    evaluate_join_terms, evaluate_range_terms, evaluate_size_terms, evaluate_sum_terms, evaluate_unary_term,
    evaluate_determinant_term, evaluate_matrix_constructor_terms, evaluate_elementwise_terms, evaluate_index_axes,
    evaluate_map_terms, evaluate_apply_terms, evaluate_apply_head_terms, evaluate_sum_iterator_terms,
    evaluate_product_iterator_terms, evaluate_product_terms, evaluate_rule_terms, evaluate_replace_all_terms,
    evaluate_matches_terms, evaluate_collect_matches_terms, evaluate_simplify_terms,
    evaluate_special_unary_terms, evaluate_extension_apply_terms, slot_as_boolean_like,
};

use athena_types::{Result, ResultId};
use athena_vm::VmConfig;

use crate::{
    domains::dispatch::DomainRequest,
    execution::{
        ir::{ExecutionModule, verify_module},
        vm::{execute_verified_cfg_on_vm_with_config, materialize_verified_vm_outcome, vm_config_from_session},
    },
    runtime::session::Session,
};

/// 供一致性测试与确定性回放共用的执行入口（现为 VM 薄包装）。
#[derive(Debug, Default)]
pub struct ReferenceExecutor {}

/// SSA 运行时槽（`athena-vm` 句柄；不与 `TermId` 共用标识域）。
pub(crate) use athena_vm::SlotValue as Slot;

impl ReferenceExecutor {
    /// 创建 reference 执行器。
    pub fn new() -> Self {
        Self {}
    }

    /// 在给定 Session / 运行时上下文中执行已校验 module。
    ///
    /// 当 `domain` 为 `Some` 时，首条 `CallProvider` 边运行 `execute_domain`
    /// 并返回该物化的 `ResultId`（IR 形态的 Goal 路径）。
    pub fn execute(&self, session: &mut Session, module: &ExecutionModule, domain: Option<DomainRequest>) -> Result<ResultId> {
        let config = vm_config_from_session(session);
        self.execute_configured(session, module, domain, &config)
    }

    /// 带 [`VmConfig`]（cancel / budget / gc_mode）的执行入口。
    ///
    /// SoftInvalid / 非布尔分支 / 缺 domain 与 VM 同合同：**硬失败**，不再软续跑。
    pub fn execute_configured(
        &self,
        session: &mut Session,
        module: &ExecutionModule,
        domain: Option<DomainRequest>,
        config: &VmConfig,
    ) -> Result<ResultId> {
        verify_module(module)?;
        let outcome = execute_verified_cfg_on_vm_with_config(session, module, domain, config)?;
        materialize_verified_vm_outcome(session, outcome, "ExecutionIR/athena-vm")
    }
}
