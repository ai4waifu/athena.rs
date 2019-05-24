//! 基于 Core IR 的重写与化简引擎。

use athena_ir::{TermNode, TermStore};
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

/// 规则驱动的 rewriter（作用于 [`TermStore`]）。
#[derive(Debug, Default)]
pub struct Rewriter {
    options: RewriteOptions,
}

impl Rewriter {
    /// 默认 rewriter。
    pub fn new() -> Self {
        Self::default()
    }

    /// 带选项构造。
    pub fn with_options(opts: RewriteOptions) -> Self {
        Self { options: opts }
    }

    /// 化简 term（stub：可选叶节点常量折叠）。
    pub fn simplify(&self, arena: &mut TermStore, root: TermId) -> Result<RewriteResult> {
        arena.verify(root)?;
        if !self.options.constant_fold {
            return Ok(RewriteResult { root, changed: false });
        }
        let changed = fold_constants(arena, root)?;
        Ok(RewriteResult { root, changed })
    }
}

fn fold_constants(arena: &mut TermStore, id: TermId) -> Result<bool> {
    // 只拷贝 `TermId` 子列表，避免复制含 `NumericValue` 的 `Atom`（Living `19`：无隐式 Clone）。
    let children: Option<Vec<TermId>> = match arena.get(id) {
        None => return Err(Diagnostic::new(DiagnosticCode::InvalidIndex)),
        Some(TermNode::List(items)) => Some(items.clone()),
        Some(TermNode::Application { arguments: args, .. }) => Some(args.clone()),
        Some(TermNode::Atom(_)) => None,
    };
    let Some(children) = children
    else {
        return Ok(false);
    };
    let mut any = false;
    for c in children {
        if fold_constants(arena, c)? {
            any = true;
        }
    }
    Ok(any)
}
