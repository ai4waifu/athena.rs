//! 关系 / scope 索引与派生索引。

pub mod derived;
pub mod index;
pub mod scope;
pub mod theory;

pub use derived::DerivedIndexes;
pub use index::{RelationIndex, RelationRecord};
pub use scope::{ScopeEdge, ScopeIndex};
