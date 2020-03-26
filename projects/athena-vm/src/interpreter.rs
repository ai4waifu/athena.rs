//! 解释执行器骨架。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    config::VmConfig,
    exit::VmExit,
    instruction::Instruction,
    module::VmModule,
};

/// VM 执行器合同。
pub trait VmExecutor {
    /// 执行模块并返回出口。
    fn execute(&mut self, module: &VmModule, config: &VmConfig) -> Result<VmExit>;
}

/// 参考解释器（正确性路径骨架）。
#[derive(Debug, Default, Clone, Copy)]
pub struct Interpreter;

impl Interpreter {
    /// 构造解释器。
    pub const fn new() -> Self {
        Self
    }
}

impl VmExecutor for Interpreter {
    fn execute(&mut self, module: &VmModule, config: &VmConfig) -> Result<VmExit> {
        if module.fingerprint != crate::module::ModuleFingerprint::of_module(module) {
            return Ok(VmExit::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "athena-vm")
                    .detail("reason", "fingerprint_mismatch"),
            ));
        }
        if module.instructions.is_empty() {
            return Ok(VmExit::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "athena-vm")
                    .detail("reason", "empty_module"),
            ));
        }

        let mut steps = 0u64;
        for insn in &module.instructions {
            steps = steps.saturating_add(1);
            if let Some(max) = config.max_steps {
                if steps > max {
                    return Ok(VmExit::BudgetExceeded);
                }
            }
            match insn {
                Instruction::Safepoint => {
                    // 骨架：仅占步。后续接 root / cancel / GcMode 检查。
                }
                Instruction::Return => return Ok(VmExit::Returned),
            }
        }

        Ok(VmExit::Diagnostic(
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "athena-vm")
                .detail("reason", "unterminated_module"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::Instruction;

    #[test]
    fn empty_return_module_exits_returned() {
        let module = VmModule::empty_return();
        let mut vm = Interpreter::new();
        let exit = vm.execute(&module, &VmConfig::default()).expect("execute");
        assert_eq!(exit, VmExit::Returned);
    }

    #[test]
    fn max_steps_budget() {
        let module = VmModule::from_instructions(
            vec![Instruction::Safepoint, Instruction::Safepoint, Instruction::Return],
            0,
        );
        let mut vm = Interpreter::new();
        let cfg = VmConfig::new().with_max_steps(1);
        let exit = vm.execute(&module, &cfg).expect("execute");
        assert_eq!(exit, VmExit::BudgetExceeded);
    }

    #[test]
    fn empty_instructions_diagnostic() {
        let module = VmModule::from_instructions(Vec::new(), 0);
        let mut vm = Interpreter::new();
        let exit = vm.execute(&module, &VmConfig::default()).expect("execute");
        assert!(matches!(exit, VmExit::Diagnostic(_)));
    }
}
