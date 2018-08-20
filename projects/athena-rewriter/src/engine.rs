//! 基于 Core IR 的重写与化简引擎。

use athena_ir::{TermArena, TermKind};
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

/// 重写 pass 选项。
#[derive(Debug, Clone, Default)]
pub struct RewriteOptions {
    /// 启用常量折叠。
    pub constant_fold: bool,
}

/// 重写 pass 结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteResult {
    /// 重写后的根 term（可与输入相同）。
    pub root: TermId,
    /// 树是否发生变化。
    pub changed: bool,
}

/// 规则驱动的 rewriter（作用于 [`TermArena`]）。
#[derive(Debug, Default)]
pub struct Rewriter {
    opts: RewriteOptions,
}

impl Rewriter {
    /// 默认 rewriter。
    pub fn new() -> Self {
        Self::default()
    }

    /// 带选项构造。
    pub fn with_options(opts: RewriteOptions) -> Self {
        Self { opts }
    }

    /// 化简 term（stub：可选叶节点常量折叠）。
    pub fn simplify(&self, arena: &mut TermArena, root: TermId) -> Result<RewriteResult> {
        arena.verify(root)?;
        if !self.opts.constant_fold {
            return Ok(RewriteResult { root, changed: false });
        }
        let changed = fold_constants(arena, root)?;
        Ok(RewriteResult { root, changed })
    }
}

fn fold_constants(arena: &mut TermArena, id: TermId) -> Result<bool> {
    let Some(kind) = arena.get(id).cloned()
    else {
        return Err(Diagnostic::error(DiagnosticCode::InvalidIndex, "invalid TermId"));
    };
    match kind {
        TermKind::List(items) => {
            let mut any = false;
            for c in items {
                if fold_constants(arena, c)? {
                    any = true;
                }
            }
            Ok(any)
        }
        TermKind::App { args, .. } => {
            let mut any = false;
            for c in &args {
                if fold_constants(arena, *c)? {
                    any = true;
                }
            }
            Ok(any)
        }
        TermKind::Atom(_) => Ok(false),
    }
}
