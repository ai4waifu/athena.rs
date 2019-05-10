//! 微积分模块共享的项改写。

use crate::{
    numeric_clone::clone_term,
    term::{Atom, Term},
};

/// 将 `var` 的每次出现替换为 `with`。
pub fn replace_symbol(expr: &Term, var: &str, with: &Term) -> Term {
    match expr {
        Term::Atom(Atom::Symbol(s)) if s == var => clone_term(with),
        Term::Atom(_) => clone_term(expr),
        Term::List(items) => Term::List(items.iter().map(|i| replace_symbol(i, var, with)).collect()),
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(replace_symbol(head, var, with)),
            arguments: args.iter().map(|a| replace_symbol(a, var, with)).collect(),
        },
    }
}

/// `var` 是否在 `expr` 中自由出现。
pub fn contains_symbol(expr: &Term, var: &str) -> bool {
    match expr {
        Term::Atom(Atom::Symbol(s)) => s == var,
        Term::Atom(_) => false,
        Term::List(items) => items.iter().any(|i| contains_symbol(i, var)),
        Term::Application { head, arguments: args } => {
            contains_symbol(head, var) || args.iter().any(|a| contains_symbol(a, var))
        }
    }
}
