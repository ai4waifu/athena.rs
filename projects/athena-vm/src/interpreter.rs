//! 解释执行器骨架。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    config::VmConfig,
    exit::VmExit,
    frame::{Frame, FrameStack},
    instruction::Instruction,
    module::VmModule,
    slot::SlotTable,
};

/// VM 执行器合同。
pub trait VmExecutor {
    /// 执行模块并返回出口。
    fn execute(&mut self, module: &VmModule, config: &VmConfig) -> Result<VmExit>;
}

/// 参考解释器（正确性路径骨架）。
#[derive(Debug, Default, Clone)]
pub struct Interpreter {
    /// 稠密槽表（跨次执行可复用容量）。
    slots: SlotTable,
    /// 帧栈。
    frames: FrameStack,
}

impl Interpreter {
    /// 构造解释器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前槽表（测试 / bridge）。
    pub fn slots(&self) -> &SlotTable {
        &self.slots
    }

    /// 当前帧栈（测试 / bridge）。
    pub fn frames(&self) -> &FrameStack {
        &self.frames
    }

    fn reset_for_module(&mut self, module: &VmModule) {
        self.slots.ensure(module.locals);
        for i in 0..module.locals {
            self.slots.clear_at(i);
        }
        self.frames = FrameStack::new();
        self.frames.push(Frame::new(0, module.locals));
    }

    fn check_budget_and_cancel(&self, steps: u64, config: &VmConfig) -> Option<VmExit> {
        if config.cancellation.is_cancelled() {
            return Some(VmExit::Cancelled);
        }
        if let Some(max) = config.max_steps {
            if steps > max {
                return Some(VmExit::BudgetExceeded);
            }
        }
        None
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

        self.reset_for_module(module);

        let mut steps = 0u64;
        for insn in &module.instructions {
            steps = steps.saturating_add(1);
            if let Some(exit) = self.check_budget_and_cancel(steps, config) {
                return Ok(exit);
            }
            if let Some(frame) = self.frames.current_mut() {
                frame.pc = frame.pc.saturating_add(1);
            }
            match insn {
                Instruction::Safepoint => {
                    // 骨架：cancel / budget 已在上方检查；后续接 root / GcMode 主动 collect。
                    let _mode = config.gc_mode;
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
    use crate::{cancel::CancellationToken, instruction::Instruction};

    #[test]
    fn empty_return_module_exits_returned() {
        let module = VmModule::empty_return();
        let mut vm = Interpreter::new();
        let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
        assert_eq!(exit, VmExit::Returned);
        assert_eq!(vm.frames().depth(), 1);
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
    fn cancel_at_safepoint() {
        let token = CancellationToken::new();
        token.cancel();
        let module = VmModule::from_instructions(vec![Instruction::Safepoint, Instruction::Return], 2);
        let mut vm = Interpreter::new();
        let cfg = VmConfig::new().with_cancellation(token);
        let exit = vm.execute(&module, &cfg).expect("execute");
        assert_eq!(exit, VmExit::Cancelled);
        assert_eq!(vm.slots().len(), 2);
    }

    #[test]
    fn empty_instructions_diagnostic() {
        let module = VmModule::from_instructions(Vec::new(), 0);
        let mut vm = Interpreter::new();
        let exit = vm.execute(&module, &VmConfig::new()).expect("execute");
        assert!(matches!(exit, VmExit::Diagnostic(_)));
    }
}
