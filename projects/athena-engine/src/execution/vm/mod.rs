//! 栈式 VM — 运行期唯一执行形态是 [`ExecUnit`]（Living `25` L2）。
//!
//! env 语义对齐 legacy 单 `DefinitionMap` 线程：任一时刻只有一个当前 env
//! （帧 + 层），作用域形式整建整拆，不与外层链接。

use std::{collections::HashMap, rc::Rc};

use athena_ir::{Atom, TermNode};
use athena_numeric::{Number, NumericContext};
use athena_types::{Diagnostic, DiagnosticCode, OperatorId, SymbolId, TermId};

use crate::runtime::{
    session::Session,
    values::arena::{push_app_named, push_bool, push_int, push_list, push_null, push_symbol_name},
};

use crate::execution::{
    EvalKind, Outcome,
    builtins::rewriting,
    compile,
    environment::definitions::{DefinitionLayer, LocalBinding, ScopeFrame},
    kernel_ir::unit::ExecUnit,
};

/// 编译模式（决定 `Set` / 作用域形式的 env 语义变体）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompileMode {
    /// 值位（非语句位）。
    Value,
    /// 语句位（继承当前 env）。
    Stmt,
    /// 顶层单语句（`With` / `Module` / `Block` 可见 Session 定义表）。
    Top,
}

/// 编译缓存：canonical hash → 候选（源子树 + 单元），命中须 `structural_eq` 复核。
#[derive(Debug, Default)]
pub struct UnitCache {
    value: HashMap<u64, Vec<(TermId, Rc<ExecUnit>)>>,
    stmt: HashMap<u64, Vec<(TermId, Rc<ExecUnit>)>>,
}

impl UnitCache {
    /// 空缓存。
    pub fn new() -> Self {
        Self::default()
    }

    fn slot(&self, mode: CompileMode) -> Option<&HashMap<u64, Vec<(TermId, Rc<ExecUnit>)>>> {
        match mode {
            CompileMode::Value => Some(&self.value),
            CompileMode::Stmt | CompileMode::Top => Some(&self.stmt),
        }
    }

    fn slot_mut(&mut self, mode: CompileMode) -> Option<&mut HashMap<u64, Vec<(TermId, Rc<ExecUnit>)>>> {
        match mode {
            CompileMode::Value => Some(&mut self.value),
            CompileMode::Stmt => Some(&mut self.stmt),
            // Top 编译不缓存（顶层作用域形式依赖 env 快照）。
            CompileMode::Top => None,
        }
    }

    fn lookup(&self, hash: u64, root: TermId, arena: &athena_ir::TermStore, mode: CompileMode) -> Option<Rc<ExecUnit>> {
        for (source, unit) in self.slot(mode)?.get(&hash)? {
            if arena.structural_eq(*source, root) {
                return Some(unit.clone());
            }
        }
        None
    }

    fn insert(&mut self, hash: u64, root: TermId, unit: ExecUnit, mode: CompileMode) {
        if let Some(slot) = self.slot_mut(mode) {
            slot.entry(hash).or_default().push((root, Rc::new(unit)));
        }
    }
}

/// 当前求值环境：作用域帧栈 + 语句定义层。
struct Env {
    /// `true` 时读取穿透到 Session 定义表（根 env / 顶层作用域形式）。
    base_global: bool,
    /// 局部定义层（fresh env / 作用域形式写入落点）。
    local: Option<DefinitionLayer>,
    frames: Vec<ScopeFrame>,
}

/// 栈式 VM。
pub(crate) struct Vm<'a> {
    pub(crate) session: &'a mut Session,
    envs: Vec<Env>,
    pub(crate) depth: u32,
    pub(crate) num_ctx: NumericContext,
}

impl<'a> Vm<'a> {
    pub(crate) fn new(session: &'a mut Session) -> Self {
        let num_ctx = session.numeric_context();
        Self { session, envs: vec![Env { base_global: true, local: None, frames: Vec::new() }], depth: 0, num_ctx }
    }

    // ---- env ----

    /// fresh env（legacy 值位特殊形式：全新空表，不与外层链接）。
    pub(crate) fn push_env_fresh(&mut self) {
        self.envs.push(Env { base_global: false, local: Some(DefinitionLayer::new()), frames: Vec::new() });
    }

    /// 作用域 env（`With` / `Module` / `Block` · `top` 决定是否可见 Session 定义表）。
    pub(crate) fn push_env_scoped(&mut self, frame: ScopeFrame, top: bool) {
        self.envs.push(Env { base_global: top, local: Some(DefinitionLayer::new()), frames: vec![frame] });
    }

    pub(crate) fn pop_env(&mut self) {
        self.envs.pop();
    }

    fn current_local_mut(&mut self) -> Option<&mut DefinitionLayer> {
        self.envs.last_mut().and_then(|e| e.local.as_mut())
    }

    /// 当前 env 写 Own（根 env 落 Session 定义表，作用域 env 落局部层并随 pop 丢弃）。
    pub(crate) fn define_own(&mut self, sym: SymbolId, value: TermId) {
        match self.current_local_mut() {
            Some(layer) => layer.define_own(sym, value),
            None => self.session.defs.define_own(sym, value),
        }
    }

    /// 当前 env 写 Delayed。
    pub(crate) fn define_delayed(&mut self, sym: SymbolId, value: TermId) {
        match self.current_local_mut() {
            Some(layer) => layer.define_delayed(sym, value),
            None => self.session.defs.define_delayed(sym, value),
        }
    }

    /// 当前 env 追加 DownValue。
    pub(crate) fn define_down_value(&mut self, sym: SymbolId, lhs: TermId, rhs: TermId) {
        match self.current_local_mut() {
            Some(layer) => layer.define_down_value(sym, lhs, rhs),
            None => self.session.defs.define_down_value(sym, lhs, rhs),
        }
    }

    /// 符号解析（帧 → 层 / Session 定义表，仅当前 env）。
    pub(crate) fn lookup_symbol(&self, sym: SymbolId) -> Option<LocalBinding> {
        let env = self.envs.last()?;
        for frame in env.frames.iter().rev() {
            if let Some(b) = frame.lookup(sym) {
                return Some(b);
            }
        }
        if env.base_global {
            if let Some(v) = self.session.defs.own(sym) {
                return Some(LocalBinding::Own(v));
            }
            if let Some(v) = self.session.defs.delayed(sym) {
                return Some(LocalBinding::Own(v));
            }
            None
        }
        else {
            let layer = env.local.as_ref()?;
            layer.own(sym).or_else(|| layer.delayed(sym)).map(LocalBinding::Own)
        }
    }

    /// 符号 DownValues（当前 env）。
    pub(crate) fn down_values(&self, sym: SymbolId) -> Option<Vec<(TermId, TermId)>> {
        let env = self.envs.last()?;
        if env.base_global {
            self.session.defs.down_values(sym).map(<[(TermId, TermId)]>::to_vec)
        }
        else {
            env.local.as_ref().and_then(|l| l.down_values(sym)).map(<[(TermId, TermId)]>::to_vec)
        }
    }

    // ---- arena 构造 ----

    pub(crate) fn rebuild_app_op(&mut self, op: OperatorId, args: Vec<TermId>) -> TermId {
        let span = TermNode::default_span();
        self.session.arena.push(TermNode::App { op, args }, span)
    }

    /// `Application[headTerm, args…]` 包装（非符号 head）。
    pub(crate) fn rebuild_app_wrapped(&mut self, args: Vec<TermId>) -> TermId {
        let op = self.session.operators.intern("Application");
        self.rebuild_app_op(op, args)
    }

    /// 惰性重建 App（head 已求值）：符号 head → `App{op:符号名}`，否则 `Application` 包装。
    pub(crate) fn rebuild_app(&mut self, head: TermId, args: Vec<TermId>) -> TermId {
        match self.session.arena.get(head) {
            Some(TermNode::Atom(Atom::Symbol(sym))) => {
                let name = self.session.arena.symbols().resolve(*sym).unwrap_or("?").to_string();
                let op = self.session.operators.intern(&name);
                self.rebuild_app_op(op, args)
            }
            _ => {
                let mut wrapped = Vec::with_capacity(args.len() + 1);
                wrapped.push(head);
                wrapped.extend(args);
                self.rebuild_app_wrapped(wrapped)
            }
        }
    }

    pub(crate) fn push_app(&mut self, head: &str, args: Vec<TermId>) -> TermId {
        push_app_named(self.session, head, args)
    }

    pub(crate) fn push_list(&mut self, items: Vec<TermId>) -> TermId {
        push_list(self.session, items)
    }

    pub(crate) fn push_int(&mut self, n: i64) -> TermId {
        push_int(self.session, n)
    }

    pub(crate) fn push_bool(&mut self, v: bool) -> TermId {
        push_bool(self.session, v)
    }

    pub(crate) fn push_null(&mut self) -> TermId {
        push_null(self.session)
    }

    pub(crate) fn push_symbol(&mut self, name: &str) -> TermId {
        push_symbol_name(self.session, name)
    }

    /// 复制数字（`clone_inline` 优先，否则挂 session heap）。
    pub(crate) fn copy_number(&self, n: &Number) -> athena_types::Result<Number> {
        match n.clone_inline() {
            Some(v) => Ok(v),
            None => n.try_clone_in(&self.num_ctx),
        }
    }

    /// head 名（App 走注册表反查 · List → `List` · 符号原子 → 自身）。
    pub(crate) fn head_name(&self, id: TermId) -> Option<String> {
        match self.session.arena.get(id)? {
            TermNode::App { op, .. } => self.session.operators.name(*op).map(str::to_string),
            TermNode::List(_) => Some("List".into()),
            TermNode::Atom(Atom::Symbol(sym)) => self.session.arena.symbols().resolve(*sym).map(str::to_string),
            _ => None,
        }
    }

    /// App 的参数列表（List 同形）。
    pub(crate) fn app_args(&self, id: TermId) -> Option<Vec<TermId>> {
        match self.session.arena.get(id)? {
            TermNode::App { args, .. } => Some(args.clone()),
            TermNode::List(items) => Some(items.clone()),
            _ => None,
        }
    }

    /// 廉价结构快照（不复制数字载荷）。
    pub(crate) fn shape(&self, id: TermId) -> Option<Shape> {
        match self.session.arena.get(id)? {
            TermNode::Atom(Atom::Number(_)) => Some(Shape::Number),
            TermNode::Atom(Atom::String(s)) => Some(Shape::Str(s.clone())),
            TermNode::Atom(Atom::Symbol(s)) => Some(Shape::Sym(*s)),
            TermNode::Atom(Atom::Boolean(b)) => Some(Shape::Bool(*b)),
            TermNode::Atom(Atom::Null) => Some(Shape::Null),
            TermNode::List(items) => Some(Shape::List(items.clone())),
            TermNode::App { op, args } => Some(Shape::App(*op, args.clone())),
        }
    }

    /// 唯一化局部符号（`name$N` 物化）。
    pub(crate) fn unique_symbol(&mut self, name: &str) -> TermId {
        self.session.module_counter += 1;
        let uniq = format!("{name}${}", self.session.module_counter);
        self.push_symbol(&uniq)
    }

    // ---- 求值入口 ----

    /// 顶层求值（语句语义 · Session 定义表可见）。
    pub fn evaluate_top(session: &'a mut Session, expr: TermId) -> Outcome {
        let mut vm = Vm::new(session);
        vm.eval(expr, CompileMode::Top)
    }

    /// 值位求值。
    pub(crate) fn eval_value(&mut self, expr: TermId) -> Outcome {
        self.eval(expr, CompileMode::Value)
    }

    /// 语句位求值（`Set` 定义、循环体继承 env）。
    pub(crate) fn eval_stmt(&mut self, expr: TermId) -> Outcome {
        self.eval(expr, CompileMode::Stmt)
    }

    fn eval(&mut self, expr: TermId, mode: CompileMode) -> Outcome {
        if self.depth > 256 {
            return Outcome::unevaluated(expr);
        }
        // 语句位的控制形式在 legacy 于 `apply_bindings` 之前被拦截：循环体必须保持原始，
        // 让每次迭代看到新绑定。跳过改写预处理，直接编译。
        let rewritten = if self.skips_rewrite(expr, mode) { expr } else { rewriting::rewrite_bindings(self, expr) };
        let unit = self.get_or_compile(rewritten, mode);
        self.depth += 1;
        let out = self.run(&unit);
        self.depth -= 1;
        out
    }

    /// 语句位控制形式与顶层作用域形式在 legacy 于改写预处理之前被拦截：
    /// 循环体保持原始让每次迭代看到新绑定；顶层 `With` / `Module` / `Block`
    /// 体不得被 Session 定义穿透。
    fn skips_rewrite(&self, expr: TermId, mode: CompileMode) -> bool {
        let Some(TermNode::App { op, .. }) = self.session.arena.get(expr)
        else {
            return false;
        };
        match self.session.operators.name(*op) {
            Some("While" | "For" | "Try" | "CompoundExpression") => mode == CompileMode::Stmt,
            Some("With" | "Module" | "Block") => mode == CompileMode::Top,
            _ => false,
        }
    }

    fn get_or_compile(&mut self, root: TermId, mode: CompileMode) -> Rc<ExecUnit> {
        let hash = athena_ir::canonical_hash(&self.session.arena, root);
        if let Some(unit) = self.session.units.lookup(hash, root, &self.session.arena, mode) {
            return unit;
        }
        let unit = compile::lower(self, root, mode);
        let rc = Rc::new(unit);
        self.session.units.insert(hash, root, (*rc).clone(), mode);
        rc
    }

    // ---- 执行 ----

    fn run(&mut self, unit: &ExecUnit) -> Outcome {
        let mut stack: Vec<(TermId, EvalKind, athena_types::ComputationStatus)> = Vec::new();
        let mut diags: Vec<Diagnostic> = Vec::new();
        let mut pc = 0usize;
        loop {
            let Some(instr) = unit.code.get(pc)
            else {
                break;
            };
            match instr {
                super::Instr::Const { term } => stack.push((*term, EvalKind::Value, athena_types::ComputationStatus::Exact)),
                super::Instr::MakeList { argc } => {
                    let n = *argc as usize;
                    let popped = stack.split_off(stack.len() - n);
                    let all_value = popped.iter().all(|(_, k, _)| *k == EvalKind::Value);
                    let items: Vec<TermId> = popped.into_iter().map(|(t, _, _)| t).collect();
                    let term = self.push_list(items);
                    let (kind, status) = if all_value {
                        (EvalKind::Value, athena_types::ComputationStatus::Exact)
                    }
                    else {
                        (EvalKind::Unevaluated, athena_types::ComputationStatus::Partial)
                    };
                    stack.push((term, kind, status));
                }
                super::Instr::MakeApp { op, argc } => {
                    let n = *argc as usize;
                    let args: Vec<TermId> = stack.split_off(stack.len() - n).into_iter().map(|(t, _, _)| t).collect();
                    let term = self.rebuild_app_op(*op, args);
                    stack.push((term, EvalKind::Unevaluated, athena_types::ComputationStatus::Unknown));
                }
                super::Instr::EvalOp { handler, argc } => {
                    let n = *argc as usize;
                    let args: Vec<TermId> = stack.split_off(stack.len() - n).into_iter().map(|(t, _, _)| t).collect();
                    let out = (super::HANDLERS[handler.0 as usize])(self, &args);
                    if out.has_error() {
                        diags.extend(out.diagnostics);
                        return Outcome {
                            term: out.term,
                            kind: EvalKind::Unevaluated,
                            status: athena_types::ComputationStatus::Invalid,
                            diagnostics: diags,
                        };
                    }
                    diags.extend(out.diagnostics);
                    stack.push((out.term, out.kind, out.status));
                }
                super::Instr::EvalRaw { handler, operands } => {
                    let out = (super::HANDLERS[handler.0 as usize])(self, operands);
                    if out.has_error() {
                        diags.extend(out.diagnostics);
                        return Outcome {
                            term: out.term,
                            kind: EvalKind::Unevaluated,
                            status: athena_types::ComputationStatus::Invalid,
                            diagnostics: diags,
                        };
                    }
                    diags.extend(out.diagnostics);
                    stack.push((out.term, out.kind, out.status));
                }
                super::Instr::EvalDynamic { argc } => {
                    let n = *argc as usize;
                    let args: Vec<TermId> = stack.split_off(stack.len() - n).into_iter().map(|(t, _, _)| t).collect();
                    let (head, _, _) = stack.pop().expect("EvalDynamic head");
                    let out = crate::execution::builtins::control::eval_dynamic(self, head, args);
                    if out.has_error() {
                        diags.extend(out.diagnostics);
                        return Outcome {
                            term: out.term,
                            kind: EvalKind::Unevaluated,
                            status: athena_types::ComputationStatus::Invalid,
                            diagnostics: diags,
                        };
                    }
                    diags.extend(out.diagnostics);
                    stack.push((out.term, out.kind, out.status));
                }
                super::Instr::BranchFalse { target } => {
                    let (t, _, _) = stack.pop().expect("BranchFalse operand");
                    if crate::execution::builtins::catalog::as_boolean_id(self, t) == Some(false) {
                        pc = *target as usize;
                        continue;
                    }
                }
                super::Instr::Jump { target } => {
                    pc = *target as usize;
                    continue;
                }
                super::Instr::DefineOwn { sym } => {
                    let (value, _, _) = stack.last().copied().expect("DefineOwn rhs");
                    self.define_own(*sym, value);
                }
                super::Instr::DefineDelayed { sym } => {
                    let (value, _, _) = stack.pop().expect("DefineDelayed rhs");
                    self.define_delayed(*sym, value);
                    stack.push((self.push_null(), EvalKind::Value, athena_types::ComputationStatus::Exact));
                }
                super::Instr::DefineDownValue { sym, lhs } => {
                    let (value, _, _) = stack.pop().expect("DefineDownValue rhs");
                    self.define_down_value(*sym, *lhs, value);
                    stack.push((self.push_null(), EvalKind::Value, athena_types::ComputationStatus::Exact));
                }
                super::Instr::Ret => {
                    let (term, kind, status) =
                        stack.pop().unwrap_or((self.push_null(), EvalKind::Value, athena_types::ComputationStatus::Exact));
                    return Outcome { term, kind, status, diagnostics: diags };
                }
            }
            pc += 1;
        }
        let (term, kind, status) =
            stack.pop().unwrap_or((self.push_null(), EvalKind::Value, athena_types::ComputationStatus::Exact));
        Outcome { term, kind, status, diagnostics: diags }
    }
}

/// 廉价结构快照（数字载荷只标记为 `Number`，不复制）。
pub(crate) enum Shape {
    Number,
    Str(String),
    Sym(SymbolId),
    Bool(bool),
    Null,
    List(Vec<TermId>),
    App(OperatorId, Vec<TermId>),
}

/// Session 顶层求值入口（语句语义 · Living `25` L2 公共门）。
pub fn evaluate_session(session: &mut Session, expr: TermId) -> Outcome {
    Vm::evaluate_top(session, expr)
}

/// handler 求值错误捷径。
pub(crate) fn invalid_echo(echo: TermId, operation: &str) -> Outcome {
    Outcome::invalid(echo, Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", operation))
}
