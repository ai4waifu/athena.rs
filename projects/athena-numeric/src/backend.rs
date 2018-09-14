//! 数值 backend 选择（骨架）。

/// 数值 backend 能力。
pub trait NumericBackend {
    /// 后端名。
    fn name(&self) -> &'static str;
    /// 是否可用于 wasm32。
    fn wasm_safe(&self) -> bool;
}

/// 默认纯 Rust backend。
#[derive(Debug, Clone, Copy, Default)]
pub struct PureRustBackend;

impl NumericBackend for PureRustBackend {
    fn name(&self) -> &'static str {
        "pure-rust"
    }

    fn wasm_safe(&self) -> bool {
        true
    }
}
