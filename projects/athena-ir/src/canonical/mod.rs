//! 规范化指纹。

pub mod fingerprint;

pub use fingerprint::{canonical_hash, canonical_hash_named, fnv1a64};
