//! 规范化指纹。

pub mod fingerprint;

pub use fingerprint::{canonical_hash, fnv1a64};
