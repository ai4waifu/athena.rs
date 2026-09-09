//! `ReferenceExecutor` — 过渡期 **host adapter** 名（暂住 engine）。
//!
//! 解释循环只走 [`crate::execution::vm::execute_verified_cfg_on_vm_with_config`]。
//! 本模块**不再**保留 `eval_region` / 第二套 CFG 语义循环。语义经
//! [`crate::execution::execution_host::ExecutionHost`]（`VmHost`）。

mod helpers;

pub(crate) use self::helpers::{
    CompareOutcome, IndexOutcome, MatrixStoreOutcome, domain_result_symbolic_term, linear_algebra_value_symbolic_term, evaluate_apply_head_terms, evaluate_apply_terms, evaluate_arithmetic_terms,
    evaluate_collect_matches_terms, evaluate_compare_terms, evaluate_determinant_term, evaluate_elementwise_terms,
    evaluate_extension_apply_terms, evaluate_index_axes, evaluate_index_axes_matrix, evaluate_join_terms, evaluate_map_indexed_terms, evaluate_map_terms,
    evaluate_map_thread_terms, evaluate_matches_terms,
    evaluate_matrix_constructor_terms, evaluate_diagonal_matrix_terms, evaluate_product_iterator_terms, evaluate_product_terms, evaluate_range_terms,
    evaluate_replace_all_terms, evaluate_rule_terms, evaluate_simplify_terms, evaluate_size_terms, evaluate_special_unary_terms,
    evaluate_sum_iterator_terms, evaluate_sum_terms, evaluate_take_terms, evaluate_drop_terms, evaluate_append_terms,
    evaluate_prepend_terms, evaluate_member_q_terms, evaluate_sort_terms, evaluate_delete_duplicates_terms,
    evaluate_count_terms, evaluate_partition_terms, evaluate_constant_array_terms, evaluate_union_terms,
    evaluate_intersection_terms, evaluate_accumulate_terms, evaluate_differences_terms, evaluate_free_q_terms,
    evaluate_extract_terms, evaluate_pad_left_terms, evaluate_riffle_terms, evaluate_position_terms, evaluate_array_terms,
    evaluate_unary_term, slot_as_boolean_like, compare_list_broadcast, store_index_axes, store_index_axes_matrix,
    domain_request_residual_term, linear_algebra_missing_binding, symbolic_term_from_value_id, parse_matrix_dims,
    term_scalar_rational_session, rational_to_term_session,
};

use athena_types::{Result, ResultId};
use athena_vm::VmConfig;

use crate::{
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
    /// 领域 Goal 载荷须已写入 module [`crate::execution::ir::ProviderCallDescriptor::payload`]。
    pub fn execute(&self, session: &mut Session, module: &ExecutionModule) -> Result<ResultId> {
        let config = vm_config_from_session(session);
        self.execute_configured(session, module, &config)
    }

    /// 带 [`VmConfig`]（cancel / budget / gc_mode）的执行入口。
    ///
    /// SoftInvalid / 非布尔分支 / 缺 payload 与 VM 同合同：**硬失败**，不再软续跑。
    pub fn execute_configured(&self, session: &mut Session, module: &ExecutionModule, config: &VmConfig) -> Result<ResultId> {
        verify_module(module)?;
        let outcome = execute_verified_cfg_on_vm_with_config(session, module, config)?;
        materialize_verified_vm_outcome(session, outcome, "ExecutionIR/athena-vm")
    }
}
