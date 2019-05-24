//! KernelIR 编译器 — `TermStore` 子树 → [`ExecUnit`]（Living `25` L2）。
//!
//! 编译期一次性遍历：`OperatorId` 分派预解析、`Set` 语句位降为定义指令、
//! 控制形式降为 raw handler 调用。未知算子降为惰性重建。

use athena_ir::{Atom, TermNode};
use athena_types::{OperatorId, TermId};

use crate::execution::{
    ids,
    kernel_ir::unit::{ExecUnit, HandlerId, Instr},
    vm::{CompileMode, Shape, Vm},
};

/// 编译一个子树。`mode` 决定 `Set` / 控制形式的 env 语义变体。
pub(crate) fn lower(vm: &mut Vm<'_>, root: TermId, mode: CompileMode) -> ExecUnit {
    let mut code = Vec::new();
    lower_into(vm, root, mode, &mut code);
    code.push(Instr::Return);
    ExecUnit { source: root, code }
}

fn lower_into(vm: &mut Vm<'_>, root: TermId, mode: CompileMode, code: &mut Vec<Instr>) {
    let Some(shape) = vm.shape(root)
    else {
        code.push(Instr::Constant { term: root });
        return;
    };
    match shape {
        Shape::Symbol(sym) => {
            let name = vm.session.arena.symbols().resolve(sym).unwrap_or("");
            let term = match name {
                "True" => vm.push_bool(true),
                "False" => vm.push_bool(false),
                "Null" => vm.push_null(),
                _ => root,
            };
            code.push(Instr::Constant { term });
        }
        Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => code.push(Instr::Constant { term: root }),
        Shape::List(items) => {
            let n = items.len().min(u16::MAX as usize) as u16;
            for item in items {
                lower_into(vm, item, CompileMode::Value, code);
            }
            code.push(Instr::MakeList { argc: n });
        }
        Shape::Application(op, args) => lower_application(vm, root, op, args, mode, code),
    }
}

fn raw(handler: HandlerId, operands: Vec<TermId>) -> Instr {
    Instr::EvalRaw { handler, operands }
}

fn lower_application(vm: &mut Vm<'_>, root: TermId, op: OperatorId, args: Vec<TermId>, mode: CompileMode, code: &mut Vec<Instr>) {
    let name = vm.session.operators.name(op).unwrap_or("").to_string();
    let argc = args.len();

    // ---- 原始操作数形式（legacy `eval_special_form` 拦截位）----
    match name.as_str() {
        "Hold" | "HoldForm" => {
            code.push(raw(ids::HOLD, vec![root]));
            return;
        }
        "Blank" | "BlankSequence" | "BlankNullSequence" | "Pattern" => {
            code.push(raw(ids::PATTERN_HOLD, vec![root]));
            return;
        }
        "If" => {
            code.push(raw(ids::IF, args));
            return;
        }
        "Which" => {
            code.push(raw(ids::WHICH, args));
            return;
        }
        "MatchQ" => {
            code.push(raw(ids::MATCH_Q, args));
            return;
        }
        "Cases" => {
            code.push(raw(ids::CASES, args));
            return;
        }
        "Table" => {
            code.push(raw(ids::TABLE, args));
            return;
        }
        "Sum" if argc <= 2 => {
            code.push(raw(ids::SUM, args));
            return;
        }
        "Product" => {
            code.push(raw(ids::PRODUCT, args));
            return;
        }
        "Try" => {
            code.push(raw(ids::TRY, args));
            return;
        }
        "Less" => {
            code.push(raw(ids::LESS_CHAIN, args));
            return;
        }
        "Greater" => {
            code.push(raw(ids::GREATER_CHAIN, args));
            return;
        }
        "LessEqual" => {
            code.push(raw(ids::LESS_EQUAL_CHAIN, args));
            return;
        }
        "GreaterEqual" => {
            code.push(raw(ids::GREATER_EQUAL_CHAIN, args));
            return;
        }
        "CompoundExpression" => {
            let h = if mode == CompileMode::Value { ids::COMPOUND_FRESH } else { ids::COMPOUND };
            code.push(raw(h, args));
            return;
        }
        "While" => {
            let h = if mode == CompileMode::Value { ids::WHILE_FRESH } else { ids::WHILE };
            code.push(raw(h, args));
            return;
        }
        "For" => {
            let h = if mode == CompileMode::Value { ids::FOR_FRESH } else { ids::FOR };
            code.push(raw(h, args));
            return;
        }
        "With" => {
            let h = if mode == CompileMode::Top { ids::WITH_TOP } else { ids::WITH };
            code.push(raw(h, args));
            return;
        }
        "Module" => {
            let h = if mode == CompileMode::Top { ids::MODULE_TOP } else { ids::MODULE };
            code.push(raw(h, args));
            return;
        }
        "Block" => {
            let h = if mode == CompileMode::Top { ids::BLOCK_TOP } else { ids::BLOCK };
            code.push(raw(h, args));
            return;
        }
        _ => {}
    }

    // ---- `Set` / `SetDelayed` 语句位降为定义指令 ----
    if (name == "Set" || name == "SetDelayed") && argc == 2 && mode != CompileMode::Value {
        lower_set_stmt(vm, &name, args, code);
        return;
    }

    // ---- 非符号 head（`Application` 包装）：求值 head 值后动态分派 ----
    if name == "Application" && !args.is_empty() {
        lower_into(vm, args[0], CompileMode::Value, code);
        for a in &args[1..] {
            lower_into(vm, *a, CompileMode::Value, code);
        }
        code.push(Instr::EvalDynamic { argc: (argc - 1).min(u16::MAX as usize) as u16 });
        return;
    }

    // ---- `Function` / `Rule`：已求值参数的惰性重建（Unevaluated）----
    if name == "Function" || name == "Rule" || name == "RuleDelayed" {
        for a in &args {
            lower_into(vm, *a, CompileMode::Value, code);
        }
        code.push(Instr::MakeApplication { op, argc: argc.min(u16::MAX as usize) as u16 });
        return;
    }

    // ---- `Set` / `SetDelayed` 值位 quirk（求值 rhs 两次 · legacy 一致）----
    if (name == "Set" || name == "SetDelayed") && argc == 2 {
        for a in &args {
            lower_into(vm, *a, CompileMode::Value, code);
        }
        code.push(Instr::EvalOp { handler: ids::SET_EVAL_RHS, argc: 2 });
        return;
    }

    // ---- 需 head 名的 handler（raw root 形式：handler 自行求值参数）----
    match name.as_str() {
        "Sin" | "Cos" | "Tan" | "Exp" | "Log" => {
            code.push(raw(ids::UNARY_TRIG, vec![root]));
            return;
        }
        "Mldivide" | "DotLeftDivide" => {
            code.push(raw(ids::MLDIVIDE, vec![root]));
            return;
        }
        "D" => {
            code.push(raw(ids::CALC_D, vec![root]));
            return;
        }
        "Integrate" => {
            code.push(raw(ids::CALC_INTEGRATE, vec![root]));
            return;
        }
        "Limit" => {
            code.push(raw(ids::CALC_LIMIT, vec![root]));
            return;
        }
        "Series" => {
            code.push(raw(ids::CALC_SERIES, vec![root]));
            return;
        }
        "DSolve" => {
            code.push(raw(ids::CALC_DSOLVE, vec![root]));
            return;
        }
        "LaplaceTransform" => {
            code.push(raw(ids::CALC_LAPLACE, vec![root]));
            return;
        }
        "Import" | "Export" | "Clear" | "Timing" => {
            code.push(raw(ids::UNSUPPORTED, vec![root]));
            return;
        }
        "error" | "Error" => {
            code.push(raw(ids::ERROR, vec![root]));
            return;
        }
        _ => {}
    }

    // ---- 已求值参数的 builtin 分派（arity 匹配才预解析，否则惰性重建）----
    if let Some(handler) = builtin_handler(vm, &name, argc) {
        for a in &args {
            lower_into(vm, *a, CompileMode::Value, code);
        }
        code.push(Instr::EvalOp { handler, argc: argc.min(u16::MAX as usize) as u16 });
        return;
    }

    // ---- 未知算子：惰性重建（Mathematica 惯性式）----
    for a in &args {
        lower_into(vm, *a, CompileMode::Value, code);
    }
    code.push(Instr::MakeApplication { op, argc: argc.min(u16::MAX as usize) as u16 });
}

/// `Set` / `SetDelayed` 语句位：lhs 形态决定定义指令，否则回退值位 quirk。
fn lower_set_stmt(vm: &mut Vm<'_>, name: &str, args: Vec<TermId>, code: &mut Vec<Instr>) {
    let lhs = args[0];
    let rhs = args[1];
    let lhs_symbol = match vm.session.arena.get(lhs) {
        Some(TermNode::Atom(Atom::Symbol(sym))) => Some(*sym),
        _ => None,
    };
    if name == "Set" {
        if let Some(sym) = lhs_symbol {
            lower_into(vm, rhs, CompileMode::Value, code);
            code.push(Instr::DefineOwn { symbol: sym });
            return;
        }
    }
    else if let Some(sym) = lhs_symbol {
        // `x := rhs`
        lower_into(vm, rhs, CompileMode::Value, code);
        code.push(Instr::DefineDelayed { symbol: sym });
        return;
    }
    else if let Some(TermNode::Application { head: op, .. }) = vm.session.arena.get(lhs) {
        // `f[x_] := rhs`
        let head_name = vm.session.operators.name(*op).unwrap_or("").to_string();
        if !head_name.is_empty() && head_name != "Application" {
            let sym = vm.session.arena.symbols_mut().intern(head_name);
            lower_into(vm, rhs, CompileMode::Value, code);
            code.push(Instr::DefineDownValue { symbol: sym, lhs });
            return;
        }
    }
    // 非 Symbol / 非 App lhs（legacy `match_set*` 不识别）→ 值位 quirk。
    for a in &args {
        lower_into(vm, *a, CompileMode::Value, code);
    }
    code.push(Instr::EvalOp { handler: ids::SET_EVAL_RHS, argc: 2 });
}

/// 已求值参数 builtin 的 arity 检查分派。
#[allow(clippy::match_same_arms)]
fn builtin_handler(_vm: &mut Vm<'_>, name: &str, argc: usize) -> Option<HandlerId> {
    Some(match (name, argc) {
        ("Plus", _) => ids::PLUS,
        ("Times", _) => ids::TIMES,
        ("Power", 2) => ids::POWER,
        ("Subtract", 2) => ids::SUBTRACT,
        ("Divide", 2) => ids::DIVIDE,
        ("DotTimes", 2) => ids::DOT_TIMES,
        ("DotDivide", 2) => ids::DOT_DIVIDE,
        ("DotPower", 2) => ids::DOT_POWER,
        ("Equal", 2) => ids::EQUAL,
        ("Unequal", 2) => ids::UNEQUAL,
        ("And", 2) => ids::AND,
        ("Or", 2) => ids::OR,
        ("Not", 1) => ids::NOT,
        ("List", _) => ids::LIST,
        ("Sqrt", 1) => ids::SQRT,
        ("Abs", 1) => ids::ABS,
        ("Factorial", 1) => ids::FACTORIAL,
        ("Simplify", 1) => ids::SIMPLIFY,
        ("Range", _) => ids::RANGE,
        ("Length", 1) => ids::LENGTH,
        ("First", 1) => ids::FIRST,
        ("Join", _) => ids::JOIN,
        ("Span", 2 | 3) => ids::SPAN,
        ("Part", 2..) => ids::PART,
        ("Apply", 2) => ids::APPLY,
        ("ReplaceAll", 2) => ids::REPLACE_ALL,
        ("Map", 2) => ids::MAP,
        ("Zeros", _) => ids::ZEROS,
        ("Ones", _) => ids::ONES,
        ("Eye" | "IdentityMatrix", _) => ids::EYE,
        ("Size" | "Dimensions", 1) => ids::SIZE,
        ("Det", 1) => ids::DET,
        ("LinearSolve", 2) => ids::LINEAR_SOLVE,
        ("Solve", 2) => ids::SOLVE,
        _ => return None,
    })
}
