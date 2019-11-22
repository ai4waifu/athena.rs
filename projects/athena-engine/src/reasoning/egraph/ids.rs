//! Stable handles inside one scope-local [`super::EGraph`].

/// Equivalence class identity (local to one E-Graph instance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EClassId(pub u32);

/// Enode identity (operator + child classes) inside one E-Graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ENodeId(pub u32);
