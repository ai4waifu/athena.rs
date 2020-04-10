//! 解释执行器骨架。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    config::VmConfig,
    constant::VmConstant,
    exit::VmExit,
    frame::{Frame, FrameStack},
    instruction::Instruction,
    module::VmModule,
    slot::{SlotTable, SlotValue},
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

    fn diagnostic(reason: &'static str) -> VmExit {
        VmExit::Diagnostic(
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "athena-vm")
                .detail("reason", reason),
        )
    }

    fn load_constant(&mut self, module: &VmModule, dst: u32, constant: u32) -> Option<VmExit> {
        let Some(value) = module.constants.get(constant as usize) else {
            return Some(Self::diagnostic("missing_constant"));
        };
        let slot = match *value {
            VmConstant::Boolean(v) => SlotValue::Boolean(v),
            VmConstant::Unit => SlotValue::Unit,
        };
        self.slots.set(dst, slot);
        None
    }

    fn move_slot(&mut self, dst: u32, src: u32) -> Option<VmExit> {
        let Some(value) = self.slots.get(src) else {
            return Some(Self::diagnostic("move_src_undefined"));
        };
        self.slots.set(dst, value);
        None
    }

    fn guard(&self, predicate: u32) -> Option<VmExit> {
        match self.slots.get(predicate) {
            Some(SlotValue::Boolean(true)) => None,
            Some(SlotValue::Boolean(false)) => Some(VmExit::Rejected),
            Some(_) => Some(Self::diagnostic("guard_not_boolean")),
            None => Some(Self::diagnostic("guard_undefined")),
        }
    }
}

impl VmExecutor for Interpreter {
    fn execute(&mut self, module: &VmModule, config: &VmConfig) -> Result<VmExit> {
        if module.fingerprint != crate::module::ModuleFingerprint::of_module(module) {
            return Ok(Self::diagnostic("fingerprint_mismatch"));
        }
        if module.instructions.is_empty() {
            return Ok(Self::diagnostic("empty_module"));
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
            match *insn {
                Instruction::Safepoint => {
                    let _mode = config.gc_mode;
                }
                Instruction::Return => return Ok(VmExit::Returned),
                Instruction::LoadConstant { dst, constant } => {
                    if let Some(exit) = self.load_constant(module, dst, constant) {
                        return Ok(exit);
                    }
                }
                Instruction::Move { dst, src } => {
                    if let Some(exit) = self.move_slot(dst, src) {
                        return Ok(exit);
                    }
                }
                Instruction::Guard { predicate } => {
                    if let Some(exit) = self.guard(predicate) {
                        return Ok(exit);
                    }
                }
                Instruction::Reject => return Ok(VmExit::Rejected),
            }
        }

        Ok(Self::diagnostic("unterminated_module"))
    }
}
