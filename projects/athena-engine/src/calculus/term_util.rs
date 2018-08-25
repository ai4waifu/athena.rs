//! Shared term rewrites for calculus modules.

use crate::term::{Atom, Term};

/// Replace every occurrence of `var` with `with`.
pub fn replace_symbol(expr: &Term, var: &str, with: &Term) -> Term {
    match expr {
        Term::Atom(Atom::Symbol(s)) if s == var => with.clone(),
        Term::Atom(_) => expr.clone(),
        Term::List(items) => Term::List(items.iter().map(|i| replace_symbol(i, var, with)).collect()),
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(replace_symbol(head, var, with)),
            arguments: args.iter().map(|a| replace_symbol(a, var, with)).collect(),
        },
    }
}

/// Whether `var` occurs free in `expr`.
pub fn contains_symbol(expr: &Term, var: &str) -> bool {
    match expr {
        Term::Atom(Atom::Symbol(s)) => s == var,
        Term::Atom(_) => false,
        Term::List(items) => items.iter().any(|i| contains_symbol(i, var)),
        Term::Application { head, arguments: args } => contains_symbol(head, var) || args.iter().any(|a| contains_symbol(a, var)),
    }
}
