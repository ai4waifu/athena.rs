//! 可执行模块合同（骨架）。

use crate::{constant::VmConstant, instruction::Instruction};

/// 与源无关的模块指纹。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ModuleFingerprint(pub u64);

impl ModuleFingerprint {
    /// 由常量、指令序列与 locals 计算稳定 FNV-1a 指纹。
    pub fn of_module(module: &VmModule) -> Self {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut hash = OFFSET;
        let mix = |h: &mut u64, byte: u8| {
            *h ^= u64::from(byte);
            *h = h.wrapping_mul(PRIME);
        };
        for b in module.locals.to_le_bytes() {
            mix(&mut hash, b);
        }
        for constant in &module.constants {
            match constant {
                VmConstant::Boolean(false) => mix(&mut hash, 1),
                VmConstant::Boolean(true) => mix(&mut hash, 2),
                VmConstant::Unit => mix(&mut hash, 3),
            }
        }
        for insn in &module.instructions {
            match insn {
                Instruction::Return => mix(&mut hash, 10),
                Instruction::Safepoint => mix(&mut hash, 11),
                Instruction::LoadConstant { dst, constant } => {
                    mix(&mut hash, 12);
                    for b in dst.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in constant.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
                Instruction::Move { dst, src } => {
                    mix(&mut hash, 13);
                    for b in dst.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in src.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
                Instruction::Guard { predicate } => {
                    mix(&mut hash, 14);
                    for b in predicate.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
                Instruction::Reject => mix(&mut hash, 15),
            }
        }
        Self(hash)
    }
}

/// 已编译、可解释执行的最小模块。
///
/// 这不是栈式字节码 VM，也不是 AST。完整 region/SSA `ExecutionModule` 仍在
/// `athena-engine::execution::ir`；本类型是抽离运行时边界的骨架合同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmModule {
    /// 线性指令序列（骨架；后续对齐 region/block）。
    pub instructions: Vec<Instruction>,
    /// 编译期常量表。
    pub constants: Vec<VmConstant>,
    /// 局部槽位数（骨架计数合同）。
    pub locals: u32,
    /// 结构指纹。
    pub fingerprint: ModuleFingerprint,
}

impl VmModule {
    /// 仅含 `Return` 的空模块。
    pub fn empty_return() -> Self {
        Self::from_parts(vec![Instruction::Return], Vec::new(), 0)
    }

    /// 用给定指令构造并刷新指纹（无常量）。
    pub fn from_instructions(instructions: Vec<Instruction>, locals: u32) -> Self {
        Self::from_parts(instructions, Vec::new(), locals)
    }

    /// 用指令 + 常量表构造并刷新指纹。
    pub fn from_parts(instructions: Vec<Instruction>, constants: Vec<VmConstant>, locals: u32) -> Self {
        let mut module = Self {
            instructions,
            constants,
            locals,
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        module
    }
}
