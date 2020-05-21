//! 解释执行器骨架。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    config::VmConfig,
    constant::VmConstant,
    exit::VmExit,
    frame::{Frame, FrameStack},
    host::{HostOutcome, NullHost, VmHost},
    instruction::{Instruction, MAX_HOST_ARGS},
    module::VmModule,
    slot::{SlotTable, SlotValue},
};

/// VM 执行器合同。
pub trait VmExecutor {
    /// 无 host 执行（内部使用 [`NullHost`]）。
    fn execute(&mut self, module: &VmModule, config: &VmConfig) -> Result<VmExit>;

    /// 带 host 回调执行（engine 综合体实现 [`VmHost`]）。
    fn execute_with_host(&mut self, module: &VmModule, config: &VmConfig, host: &mut dyn VmHost) -> Result<VmExit>;
}

/// 参考解释器（正确性路径骨架）。
#[derive(Debug, Default, Clone)]
pub struct Interpreter {
    /// 稠密槽表（跨次执行可复用容量）。
    slots: SlotTable,
    /// 帧栈。
    frames: FrameStack,
    /// 最近一次 [`Instruction::ReturnValue`] 的结果槽。
    last_return_slot: Option<u32>,
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

    /// 最近一次带值返回的结果槽。
    pub fn last_return_slot(&self) -> Option<u32> {
        self.last_return_slot
    }

    fn reset_for_module(&mut self, module: &VmModule) {
        self.slots.ensure(module.locals);
        for i in 0..module.locals {
            self.slots.clear_at(i);
        }
        self.frames = FrameStack::new();
        self.frames.push(Frame::new(0, module.locals));
        self.last_return_slot = None;
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
            VmConstant::Term(term) => SlotValue::Term(term),
            VmConstant::Symbol(symbol) => SlotValue::Symbol(symbol),
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

    fn collect_args(&self, argc: u8, args: &[u32; MAX_HOST_ARGS]) -> core::result::Result<[SlotValue; MAX_HOST_ARGS], VmExit> {
        let n = argc as usize;
        if n > MAX_HOST_ARGS {
            return Err(Self::diagnostic("host_argc_overflow"));
        }
        let mut out = [SlotValue::Empty; MAX_HOST_ARGS];
        for i in 0..n {
            let Some(value) = self.slots.get(args[i]) else {
                return Err(Self::diagnostic("host_arg_undefined"));
            };
            out[i] = value;
        }
        Ok(out)
    }

    fn apply_host_outcome(&mut self, dst: u32, outcome: HostOutcome) -> Option<VmExit> {
        match outcome {
            HostOutcome::Value(value) | HostOutcome::Residual(value) => {
                self.slots.set(dst, value);
                None
            }
            HostOutcome::Diagnostic(diagnostic) => Some(VmExit::Diagnostic(diagnostic)),
        }
    }
}

impl VmExecutor for Interpreter {
    fn execute(&mut self, module: &VmModule, config: &VmConfig) -> Result<VmExit> {
        let mut host = NullHost;
        self.execute_with_host(module, config, &mut host)
    }

    fn execute_with_host(&mut self, module: &VmModule, config: &VmConfig, host: &mut dyn VmHost) -> Result<VmExit> {
        if module.fingerprint != crate::module::ModuleFingerprint::of_module(module) {
            return Ok(Self::diagnostic("fingerprint_mismatch"));
        }
        if module.instructions.is_empty() {
            return Ok(Self::diagnostic("empty_module"));
        }

        self.reset_for_module(module);

        let mut steps = 0u64;
        let mut pc: u32 = 0;
        let len = module.instructions.len() as u32;
        while (pc as usize) < module.instructions.len() {
            steps = steps.saturating_add(1);
            if let Some(exit) = self.check_budget_and_cancel(steps, config) {
                return Ok(exit);
            }
            if let Some(frame) = self.frames.current_mut() {
                frame.pc = pc;
            }
            let insn = module.instructions[pc as usize];
            match insn {
                Instruction::Safepoint => {
                    let _mode = config.gc_mode;
                    pc = pc.saturating_add(1);
                }
                Instruction::Return => return Ok(VmExit::Returned),
                Instruction::ReturnValue { slot } => {
                    self.last_return_slot = Some(slot);
                    return Ok(VmExit::Returned);
                }
                Instruction::Jump { target } => {
                    if target >= len {
                        return Ok(Self::diagnostic("jump_oob"));
                    }
                    pc = target;
                }
                Instruction::Branch { condition, then_pc, else_pc } => {
                    let next = match self.slots.get(condition) {
                        Some(SlotValue::Boolean(true)) => then_pc,
                        Some(SlotValue::Boolean(false)) => else_pc,
                        Some(_) => return Ok(Self::diagnostic("branch_not_boolean")),
                        None => return Ok(Self::diagnostic("branch_undefined")),
                    };
                    if next >= len {
                        return Ok(Self::diagnostic("branch_oob"));
                    }
                    pc = next;
                }
                Instruction::LoadConstant { dst, constant } => {
                    if let Some(exit) = self.load_constant(module, dst, constant) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::Move { dst, src } => {
                    if let Some(exit) = self.move_slot(dst, src) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::Guard { predicate } => {
                    if let Some(exit) = self.guard(predicate) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::Reject => return Ok(VmExit::Rejected),
                Instruction::ApplySemantic { dst, op, argc, args } => {
                    let values = match self.collect_args(argc, &args) {
                        Ok(v) => v,
                        Err(exit) => return Ok(exit),
                    };
                    let outcome = host.apply_semantic(op, &values[..argc as usize])?;
                    if let Some(exit) = self.apply_host_outcome(dst, outcome) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::CallProvider { dst, op, argc, args } => {
                    let values = match self.collect_args(argc, &args) {
                        Ok(v) => v,
                        Err(exit) => return Ok(exit),
                    };
                    let outcome = host.call_provider(op, &values[..argc as usize])?;
                    if let Some(exit) = self.apply_host_outcome(dst, outcome) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::ReadBinding { dst, key } => {
                    let key_value = match self.slots.get(key) {
                        Some(v) => v,
                        None => return Ok(Self::diagnostic("read_binding_key_undefined")),
                    };
                    let outcome = host.read_binding(key_value)?;
                    if let Some(exit) = self.apply_host_outcome(dst, outcome) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::WriteBinding {
                    dst,
                    key,
                    value,
                    kind,
                    evaluation,
                } => {
                    let key_value = match self.slots.get(key) {
                        Some(v) => v,
                        None => return Ok(Self::diagnostic("write_binding_key_undefined")),
                    };
                    let value_slot = match self.slots.get(value) {
                        Some(v) => v,
                        None => return Ok(Self::diagnostic("write_binding_value_undefined")),
                    };
                    let outcome = host.write_binding(key_value, value_slot, kind, evaluation)?;
                    if let Some(exit) = self.apply_host_outcome(dst, outcome) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::EnterScope { dst, parent } => {
                    let parent_value = match parent {
                        Some(slot) => match self.slots.get(slot) {
                            Some(v) => Some(v),
                            None => return Ok(Self::diagnostic("enter_scope_parent_undefined")),
                        },
                        None => None,
                    };
                    let outcome = host.enter_scope(parent_value)?;
                    if let Some(exit) = self.apply_host_outcome(dst, outcome) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::ExitScope { scope } => {
                    let scope_value = match self.slots.get(scope) {
                        Some(v) => v,
                        None => return Ok(Self::diagnostic("exit_scope_undefined")),
                    };
                    let outcome = host.exit_scope(scope_value)?;
                    match outcome {
                        HostOutcome::Diagnostic(diagnostic) => return Ok(VmExit::Diagnostic(diagnostic)),
                        HostOutcome::Value(_) | HostOutcome::Residual(_) => {}
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::ConstructCollection { dst, kind, argc, args } => {
                    let values = match self.collect_args(argc, &args) {
                        Ok(v) => v,
                        Err(exit) => return Ok(exit),
                    };
                    let outcome = host.construct_collection(kind, &values[..argc as usize])?;
                    if let Some(exit) = self.apply_host_outcome(dst, outcome) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
                Instruction::Index { dst, target, axes } => {
                    let target_value = match self.slots.get(target) {
                        Some(v) => v,
                        None => return Ok(Self::diagnostic("index_target_undefined")),
                    };
                    let outcome = host.apply_index(axes, target_value)?;
                    if let Some(exit) = self.apply_host_outcome(dst, outcome) {
                        return Ok(exit);
                    }
                    pc = pc.saturating_add(1);
                }
            }
        }

        Ok(Self::diagnostic("unterminated_module"))
    }
}
