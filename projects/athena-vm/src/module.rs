//! 可执行模块合同（骨架）。

use crate::instruction::Instruction;

/// 与源无关的模块指纹。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ModuleFingerprint(pub u64);

impl ModuleFingerprint {
    /// 由指令序列与 locals 计算稳定 FNV-1a 指纹。
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
        for insn in &module.instructions {
            let tag = match insn {
                Instruction::Return => 1u8,
                Instruction::Safepoint => 2u8,
            };
            mix(&mut hash, tag);
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
    /// 局部槽位数（骨架计数合同）。
    pub locals: u32,
    /// 结构指纹。
    pub fingerprint: ModuleFingerprint,
}

impl VmModule {
    /// 仅含 `Return` 的空模块。
    pub fn empty_return() -> Self {
        let mut module = Self {
            instructions: vec![Instruction::Return],
            locals: 0,
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        module
    }

    /// 用给定指令构造并刷新指纹。
    pub fn from_instructions(instructions: Vec<Instruction>, locals: u32) -> Self {
        let mut module = Self {
            instructions,
            locals,
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        module
    }
}
