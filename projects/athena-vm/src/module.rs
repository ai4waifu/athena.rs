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
                VmConstant::Term(term) => {
                    mix(&mut hash, 4);
                    for b in term.0.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
                VmConstant::Symbol(symbol) => {
                    mix(&mut hash, 5);
                    for b in symbol.0.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
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
                Instruction::Jump { target } => {
                    mix(&mut hash, 18);
                    for b in target.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
                Instruction::Branch { condition, then_pc, else_pc } => {
                    mix(&mut hash, 19);
                    for b in condition.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in then_pc.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in else_pc.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
                Instruction::ReturnValue { slot } => {
                    mix(&mut hash, 20);
                    for b in slot.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
                Instruction::ApplySemantic { dst, op, argc, args } => {
                    mix(&mut hash, 16);
                    for b in dst.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in op.0.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    mix(&mut hash, *argc);
                    for i in 0..(*argc as usize).min(crate::instruction::MAX_HOST_ARGS) {
                        for b in args[i].to_le_bytes() {
                            mix(&mut hash, b);
                        }
                    }
                }
                Instruction::CallProvider { dst, op, argc, args } => {
                    mix(&mut hash, 17);
                    for b in dst.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in op.0.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    mix(&mut hash, *argc);
                    for i in 0..(*argc as usize).min(crate::instruction::MAX_HOST_ARGS) {
                        for b in args[i].to_le_bytes() {
                            mix(&mut hash, b);
                        }
                    }
                }
                Instruction::ReadBinding { dst, key } => {
                    mix(&mut hash, 21);
                    for b in dst.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in key.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
                Instruction::WriteBinding {
                    dst,
                    key,
                    value,
                    kind,
                    evaluation,
                } => {
                    mix(&mut hash, 22);
                    for b in dst.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in key.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in value.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    mix(&mut hash, binding_kind_tag(*kind));
                    mix(&mut hash, binding_eval_tag(*evaluation));
                }
                Instruction::EnterScope { dst, parent } => {
                    mix(&mut hash, 23);
                    for b in dst.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    match parent {
                        None => mix(&mut hash, 0),
                        Some(slot) => {
                            mix(&mut hash, 1);
                            for b in slot.to_le_bytes() {
                                mix(&mut hash, b);
                            }
                        }
                    }
                }
                Instruction::ExitScope { scope } => {
                    mix(&mut hash, 24);
                    for b in scope.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
                Instruction::ConstructCollection { dst, kind, argc, args } => {
                    mix(&mut hash, 25);
                    for b in dst.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    mix(&mut hash, collection_kind_tag(*kind));
                    mix(&mut hash, *argc);
                    for i in 0..(*argc as usize).min(crate::instruction::MAX_HOST_ARGS) {
                        for b in args[i].to_le_bytes() {
                            mix(&mut hash, b);
                        }
                    }
                }
                Instruction::Index { dst, target, axes } => {
                    mix(&mut hash, 26);
                    for b in dst.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in target.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                    for b in axes.0.to_le_bytes() {
                        mix(&mut hash, b);
                    }
                }
            }
        }
        Self(hash)
    }
}

fn binding_kind_tag(kind: athena_types::BindingKind) -> u8 {
    use athena_types::BindingKind::*;
    match kind {
        Lexical => 1,
        Dynamic => 2,
        Session => 3,
        Persistent => 4,
        Memoized => 5,
        Dispatch => 6,
    }
}

fn binding_eval_tag(evaluation: athena_types::BindingEvaluationPolicy) -> u8 {
    use athena_types::BindingEvaluationPolicy::*;
    match evaluation {
        EvaluateBeforeStore => 1,
        StoreResidualTerm => 2,
        EvaluateOnRead => 3,
        EvaluateOnApply => 4,
        MemoizeOnFirstRead => 5,
        ExplicitMaterialization => 6,
    }
}

fn collection_kind_tag(kind: athena_types::CollectionKind) -> u8 {
    use athena_types::CollectionKind::*;
    match kind {
        StructuralSequence => 1,
        Tuple => 2,
        OrderedCollection => 3,
        SetLikeCollection => 4,
        Vector => 5,
        MatrixRow => 6,
        MatrixColumn => 7,
        Matrix => 8,
        DomainCollection(id) => {
            let _ = id;
            9
        }
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
