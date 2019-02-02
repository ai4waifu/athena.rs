//! 算法策略层（schoolbook / Karatsuba / Toom / BZ / half-GCD 等）。
//!
//! 与 [`crate::kernel`]（机器指令实现）正交：本层选**哪种数学算法**，
//! kernel 只提供已绑定的 limb 原语写入。
//!
//! 当前乘法/除法/gcd 策略仍寄居在 `kernel::pure_rust::limb_kernel`，
//! 抽离到本目录是后续切片；此处保留模块边界以免再次糊回笼统 backend。
